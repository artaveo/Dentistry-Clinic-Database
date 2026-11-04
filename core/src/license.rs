//! License, trial and read-only state (roadmap 10.8-10.9, ADR-15, ADR-25).
//!
//! * The license is a *token* (JWS, Ed25519) made by the Artaveo Control Center. The app holds only
//!   the public keys, so a token cannot be forged on the clinic's computer.
//! * A token is bound to this installation (`installation_id`), this hardware (`fingerprint`) and, when
//!   it comes from the server, to the request that asked for it (`nonce`). The vendor approves each
//!   computer by hand in the panel; until then the token says `pending` and the app is read-only.
//! * Every number of the rules comes from the token (`check_every_days`, `offline_grace_days`,
//!   `post_expiry_grace_days`, `expires_at`, `valid_until`), so the vendor changes them without a new app.
//! * The token and the key live in files *beside* the database, never inside it: restoring an old backup
//!   can not bring back an expired trial or a revoked license.
//! * Read-only (10.9) refuses changes but never data access: viewing, printing, export, backup and
//!   restore keep working. A clock set back is noticed (`install.json` remembers the latest time seen).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration as StdDuration;

use artaveo_shared::{ErrorCode, LicenseState, LicenseStatus, ValidationRule};
use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::{Duration, OffsetDateTime};

use crate::clock::{iso, now, parse};
use crate::error::{CoreError, Result};
use crate::fingerprint;
use crate::ids::new_id;

/// Public keys of the Control Center: `(kid, x of the Ed25519 JWK, development only)`.
/// The development key is refused by a production build.
pub const PUBLIC_KEYS: &[(&str, &str, bool)] = &[
    ("cc-production-2026-10", "Pj91-jFZ36j_hsQsA2wex1lkdGBUz8PH0R2ueOavkXU", false),
    ("cc-development-2026-10", "2cGPw_GdOll38C3-W4xeb--VlPABsWHwkdfTcHBfp1s", true),
];

/// The production `validate` function of the Control Center (ADR-25).
pub const PRODUCTION_URL: &str =
    "https://br-calm-hill-b145m2f2-validate.compute.c-5.eu-central-1.aws.neon.tech/";

/// A clock this far behind the latest time seen counts as tampering.
const CLOCK_SLACK_HOURS: i64 = 24;
/// `install.json` is rewritten at most this often when only the "latest time seen" moved.
const SEEN_WRITE_EVERY_MINUTES: i64 = 10;

pub type Keys = Vec<(String, [u8; 32])>;

/// The keys this build trusts. `allow_development` is false in a production build.
pub fn trusted_keys(allow_development: bool) -> Keys {
    PUBLIC_KEYS
        .iter()
        .filter(|(_, _, dev)| allow_development || !dev)
        .filter_map(|(kid, x, _)| {
            let bytes: [u8; 32] = BASE64URL_NOPAD.decode(x.as_bytes()).ok()?.try_into().ok()?;
            Some((kid.to_string(), bytes))
        })
        .collect()
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Payload {
    /// `valid`, `pending`, `blocked`, `revoked`, `license_suspended`, ...
    pub result: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub plan: String,
    #[serde(default)]
    pub max_computers: u32,
    pub installation_id: String,
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub nonce: Option<String>,
    pub issued_at: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default = "default_check_days")]
    pub check_every_days: i64,
    #[serde(default = "default_offline_days")]
    pub offline_grace_days: i64,
    #[serde(default)]
    pub post_expiry_grace_days: i64,
    #[serde(default)]
    pub valid_until: Option<String>,
}

fn default_check_days() -> i64 {
    30
}

fn default_offline_days() -> i64 {
    120
}

/// What this computer remembers about itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Install {
    pub installation_id: String,
    pub first_run_at: String,
    /// The latest time ever seen on this computer's clock (clock-tamper check).
    pub last_seen_at: String,
}

/// The token as far as it could be read.
#[derive(Debug, Clone)]
pub enum Loaded {
    None,
    /// Signature wrong, file damaged, or made for another computer.
    Invalid,
    Valid(Box<Payload>),
}

/// Checks a compact JWS (`header.payload.signature`, EdDSA) and returns its payload.
pub fn verify_token(keys: &Keys, token: &str) -> Option<Payload> {
    let mut parts = token.trim().split('.');
    let (h, p, s) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let header: Value = serde_json::from_slice(&BASE64URL_NOPAD.decode(h.as_bytes()).ok()?).ok()?;
    if header["alg"] != "EdDSA" {
        return None;
    }
    let kid = header["kid"].as_str()?;
    let (_, key) = keys.iter().find(|(k, _)| k == kid)?;
    let key = VerifyingKey::from_bytes(key).ok()?;
    let sig = Signature::from_slice(&BASE64URL_NOPAD.decode(s.as_bytes()).ok()?).ok()?;
    key.verify(format!("{h}.{p}").as_bytes(), &sig).ok()?;
    serde_json::from_slice(&BASE64URL_NOPAD.decode(p.as_bytes()).ok()?).ok()
}

fn days_between(later: OffsetDateTime, earlier: OffsetDateTime) -> i64 {
    (later - earlier).whole_days()
}

fn ceil_days(until: OffsetDateTime, at: OffsetDateTime) -> i64 {
    ((until - at).whole_seconds() as f64 / 86_400.0).ceil() as i64
}

/// The pure rule: what state is this computer in right now? (Everything the tests need.)
pub fn evaluate(
    install: &Install,
    loaded: &Loaded,
    at: OffsetDateTime,
    fingerprint: &str,
    server_configured: bool,
    allow_unlicensed: bool,
) -> LicenseStatus {
    let mut s = LicenseStatus {
        state: LicenseState::Active,
        read_only: false,
        kind: None,
        plan: None,
        computers: 1,
        installation_id: install.installation_id.clone(),
        fingerprint_hint: fingerprint::short(fingerprint),
        expires_at: None,
        days_left: None,
        verified_at: None,
        verify_due: false,
        grace_days_left: None,
        server_configured,
    };
    let stop = |mut s: LicenseStatus, state: LicenseState| {
        s.state = state;
        s.read_only = true;
        s
    };
    if parse(&install.last_seen_at).is_some_and(|seen| at < seen - Duration::hours(CLOCK_SLACK_HOURS)) {
        return stop(s, LicenseState::ClockTampered);
    }
    let p = match loaded {
        Loaded::None => {
            s.state = LicenseState::Unactivated;
            s.read_only = !allow_unlicensed;
            return s;
        }
        Loaded::Invalid => return stop(s, LicenseState::Invalid),
        Loaded::Valid(p) => p,
    };
    if p.installation_id != install.installation_id || p.fingerprint != fingerprint {
        return stop(s, LicenseState::Invalid);
    }
    s.kind = Some(p.kind.clone());
    s.plan = Some(p.plan.clone());
    s.computers = p.max_computers.max(1);
    s.expires_at = p.expires_at.clone();
    s.verified_at = Some(p.issued_at.clone());
    match p.result.as_str() {
        "valid" => {}
        "pending" => return stop(s, LicenseState::Pending),
        "blocked" => return stop(s, LicenseState::Blocked),
        "license_expired" => return stop(s, LicenseState::Expired),
        "revoked" | "license_revoked" | "license_suspended" | "license_draft" | "account_inactive" => {
            return stop(s, LicenseState::Revoked)
        }
        _ => return stop(s, LicenseState::Invalid),
    }
    let (Some(issued), expires) = (parse(&p.issued_at), p.expires_at.as_deref().and_then(parse)) else {
        return stop(s, LicenseState::Invalid);
    };
    let valid_until =
        p.valid_until.as_deref().and_then(parse).unwrap_or(issued + Duration::days(p.offline_grace_days));
    let expired = expires.is_some_and(|e| at > e);
    s.days_left = expires.map(|e| ceil_days(e, at));
    if at > valid_until {
        // Past the end date: the vendor's grace is over. Otherwise the online check is what lapsed.
        s.verify_due = true;
        return stop(s, if expired { LicenseState::Expired } else { LicenseState::NeedsVerification });
    }
    s.verify_due = days_between(at, issued) >= p.check_every_days;
    if s.verify_due || expired {
        s.grace_days_left = Some(ceil_days(valid_until, at).max(0));
    }
    s.state = if expired {
        LicenseState::ExpiredWarning
    } else if s.verify_due {
        LicenseState::Grace
    } else if p.kind == "trial" {
        LicenseState::Trial
    } else {
        LicenseState::Active
    };
    s
}

/// Whether a method changes clinic data and is therefore refused in read-only state (10.9).
/// Looking, printing, exporting, reports, search, notifications, backup, restore and the license
/// pages themselves always work.
pub fn blocked_when_read_only(method: &str) -> bool {
    let Some((topic, action)) = method.split_once('.') else { return false };
    const ALWAYS: &[&str] = &["backup", "license", "reports", "search", "notifications", "dashboard"];
    if ALWAYS.contains(&topic) || action.ends_with("mark_printed") {
        return false;
    }
    crate::lan::is_change(method)
}

// ───────────────────────────── the license server ─────────────────────────────

#[derive(Debug)]
pub enum ServerError {
    /// No internet, or this build does not know where the server is.
    Unavailable(String),
    /// The server answered "no" without a token, with its reason (`unknown_key`, `invalid_request`, ...).
    Rejected(String),
}

pub trait LicenseServer: Send + Sync {
    fn configured(&self) -> bool;
    /// POSTs `{license_key, installation_id, app_version, fingerprint, computer_name, nonce}`.
    fn validate(&self, body: &Value) -> std::result::Result<Value, ServerError>;
}

/// The Control Center's `validate` function over HTTPS. A production build knows its address; other
/// builds read `ARTAVEO_LICENSE_URL` (at run time or build time). No secret is in the program.
pub struct HttpServer {
    url: Option<String>,
}

impl HttpServer {
    pub fn from_build(production: bool) -> Self {
        let url = std::env::var("ARTAVEO_LICENSE_URL")
            .ok()
            .or_else(|| option_env!("ARTAVEO_LICENSE_URL").map(str::to_string))
            .or_else(|| production.then(|| PRODUCTION_URL.to_string()))
            .filter(|v| !v.trim().is_empty());
        Self { url }
    }
}

impl LicenseServer for HttpServer {
    fn configured(&self) -> bool {
        self.url.is_some()
    }

    fn validate(&self, body: &Value) -> std::result::Result<Value, ServerError> {
        let Some(url) = &self.url else { return Err(ServerError::Unavailable("no server address".into())) };
        let tls = native_tls::TlsConnector::new().map_err(|e| ServerError::Unavailable(e.to_string()))?;
        let agent = ureq::AgentBuilder::new()
            .tls_connector(std::sync::Arc::new(tls))
            .timeout(StdDuration::from_secs(15))
            .build();
        let read = |text: Option<String>| -> std::result::Result<Value, ServerError> {
            let v: Value = text
                .and_then(|t| serde_json::from_str(&t).ok())
                .ok_or_else(|| ServerError::Unavailable("unreadable answer".into()))?;
            if v.get("token").and_then(Value::as_str).is_some() {
                Ok(v)
            } else {
                let why = v
                    .get("result")
                    .or_else(|| v.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("server_error");
                Err(if why == "server_error" {
                    ServerError::Unavailable(why.into())
                } else {
                    ServerError::Rejected(why.into())
                })
            }
        };
        match agent.post(url).set("content-type", "application/json").send_string(&body.to_string()) {
            Ok(resp) => read(resp.into_string().ok()),
            Err(ureq::Error::Status(_, resp)) => read(resp.into_string().ok()),
            Err(e) => Err(ServerError::Unavailable(e.to_string())),
        }
    }
}

/// What the app tells the server about itself (never anything about patients).
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub clinic_name: String,
    pub computer_name: String,
    pub app_version: String,
    pub arch: String,
    pub last_backup_at: Option<String>,
    pub last_backup_ok: Option<bool>,
}

// ───────────────────────────── the manager ─────────────────────────────

struct State {
    install: Install,
    loaded: Loaded,
    /// When `install.json` was last written.
    written_at: OffsetDateTime,
}

pub struct LicenseManager {
    dir: PathBuf,
    keys: Mutex<Keys>,
    server: Mutex<Box<dyn LicenseServer>>,
    state: Mutex<State>,
    /// Last time the background check tried to reach the server.
    last_try: Mutex<Option<OffsetDateTime>>,
    fingerprint: String,
    /// Development and tests work without a license; a production build does not.
    allow_unlicensed: bool,
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

impl LicenseManager {
    pub fn new(
        dir: &Path,
        keys: Keys,
        server: Box<dyn LicenseServer>,
        fingerprint: String,
        allow_unlicensed: bool,
    ) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let at = now();
        let install_path = dir.join("install.json");
        let install =
            match fs::read(&install_path).ok().and_then(|b| serde_json::from_slice::<Install>(&b).ok()) {
                Some(i) => i,
                None => {
                    let i =
                        Install { installation_id: new_id(), first_run_at: iso(at), last_seen_at: iso(at) };
                    write_atomic(&install_path, &serde_json::to_vec_pretty(&i)?)?;
                    i
                }
            };
        let m = Self {
            dir: dir.to_path_buf(),
            keys: Mutex::new(keys),
            server: Mutex::new(server),
            state: Mutex::new(State { install, loaded: Loaded::None, written_at: at }),
            last_try: Mutex::new(None),
            fingerprint,
            allow_unlicensed,
        };
        m.reload();
        Ok(m)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn keys(&self) -> Keys {
        self.keys.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn token_path(&self) -> PathBuf {
        self.dir.join("license.json")
    }

    fn key_path(&self) -> PathBuf {
        self.dir.join("license_key.txt")
    }

    /// Re-reads the token file (after it was written, or by a test).
    fn reload(&self) {
        let keys = self.keys();
        let mut st = self.lock();
        let installation = st.install.installation_id.clone();
        st.loaded = match fs::read(self.token_path()) {
            Err(_) => Loaded::None,
            Ok(bytes) => {
                let token = serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .and_then(|v| v.get("token").and_then(Value::as_str).map(str::to_string));
                match token.and_then(|t| verify_token(&keys, &t)) {
                    Some(p) if p.installation_id == installation && p.fingerprint == self.fingerprint => {
                        Loaded::Valid(Box::new(p))
                    }
                    _ => Loaded::Invalid,
                }
            }
        };
    }

    #[cfg(test)]
    pub fn set_server(&self, server: Box<dyn LicenseServer>) {
        *self.server.lock().unwrap() = server;
    }

    #[cfg(test)]
    pub fn set_keys(&self, keys: Keys) {
        *self.keys.lock().unwrap() = keys;
        self.reload();
    }

    #[cfg(test)]
    pub fn installation_id(&self) -> String {
        self.lock().install.installation_id.clone()
    }

    #[cfg(test)]
    pub fn fingerprint(&self) -> String {
        self.fingerprint.clone()
    }

    fn server_configured(&self) -> bool {
        self.server.lock().unwrap_or_else(|p| p.into_inner()).configured()
    }

    fn save_install(&self, st: &mut State) -> Result<()> {
        write_atomic(&self.dir.join("install.json"), &serde_json::to_vec_pretty(&st.install)?)?;
        st.written_at = now();
        Ok(())
    }

    /// The state right now. Also moves "latest time seen" forward.
    pub fn status(&self) -> LicenseStatus {
        self.status_at(now())
    }

    pub fn status_at(&self, at: OffsetDateTime) -> LicenseStatus {
        let configured = self.server_configured();
        let mut st = self.lock();
        let result =
            evaluate(&st.install, &st.loaded, at, &self.fingerprint, configured, self.allow_unlicensed);
        // Only a clock that is not behind may move the mark, and the file is not rewritten on every request.
        if result.state != LicenseState::ClockTampered
            && parse(&st.install.last_seen_at).is_some_and(|seen| at > seen)
        {
            st.install.last_seen_at = iso(at);
            if at - st.written_at >= Duration::minutes(SEEN_WRITE_EVERY_MINUTES) {
                if let Err(e) = self.save_install(&mut st) {
                    tracing::warn!(error = %e, "could not save install.json");
                }
            }
        }
        result
    }

    pub fn is_read_only(&self) -> bool {
        self.status().read_only
    }

    /// The background check (every few minutes from the scheduler): true when a periodic check is due
    /// and the server could be asked, but not more than once an hour (a clinic without internet must
    /// not make a failing request every five minutes).
    pub fn should_check_now(&self) -> bool {
        let s = self.status();
        if !s.server_configured || !s.verify_due || !self.key_path().exists() {
            return false;
        }
        let at = now();
        let mut last = self.last_try.lock().unwrap_or_else(|p| p.into_inner());
        if last.is_some_and(|t| at - t < Duration::hours(1)) {
            return false;
        }
        *last = Some(at);
        true
    }

    /// Asks the Control Center and keeps the signed answer. The answer must be for this request
    /// (`nonce`), this installation and this hardware, or it is refused.
    fn check_online(&self, key: &str, info: &ClientInfo) -> Result<()> {
        let mut raw = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        let nonce = HEXLOWER.encode(&raw);
        let installation = self.lock().install.installation_id.clone();
        let body = json!({
            "license_key": key,
            "installation_id": installation,
            "app_version": info.app_version,
            "fingerprint": self.fingerprint,
            "computer_name": info.computer_name,
            "nonce": nonce,
        });
        let answer =
            self.server.lock().unwrap_or_else(|p| p.into_inner()).validate(&body).map_err(|e| match e {
                ServerError::Unavailable(d) => CoreError::api(ErrorCode::LicenseServerUnavailable, d),
                ServerError::Rejected(code) => CoreError::api(ErrorCode::LicenseRejected, code),
            })?;
        let token = answer["token"].as_str().unwrap_or_default();
        let ok = verify_token(&self.keys(), token).is_some_and(|p| {
            p.nonce.as_deref() == Some(nonce.as_str())
                && p.installation_id == installation
                && p.fingerprint == self.fingerprint
        });
        if !ok {
            return Err(CoreError::api(ErrorCode::LicenseRejected, "token_invalid"));
        }
        write_atomic(&self.token_path(), &serde_json::to_vec_pretty(&json!({ "token": token }))?)?;
        write_atomic(&self.key_path(), key.as_bytes())?;
        self.reload();
        Ok(())
    }

    /// Entering the license key (roadmap 10.8). The computer then waits for the vendor's approval.
    pub fn activate(&self, code: &str, info: &ClientInfo) -> Result<LicenseStatus> {
        let key: String = code.trim().to_ascii_uppercase();
        let well_formed = key.len() == 29
            && key.split('-').count() == 6
            && key.split('-').all(|g| g.len() == 4 && g.chars().all(|c| c.is_ascii_hexdigit()));
        if !well_formed {
            return Err(CoreError::invalid(
                "code",
                ValidationRule::ActivationCodeFormat,
                "a license key looks like 0A1B-2C3D-4E5F-6A7B-8C9D-0E1F",
            ));
        }
        self.check_online(&key, info)?;
        Ok(self.status())
    }

    /// The periodic check (default every 30 days while online) with the stored key.
    pub fn verify(&self, info: &ClientInfo) -> Result<LicenseStatus> {
        let key = fs::read_to_string(self.key_path())
            .ok()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .ok_or_else(|| CoreError::api(ErrorCode::LicenseRejected, "no_key"))?;
        self.check_online(&key, info)?;
        Ok(self.status())
    }

    /// The short text a clinic without internet sends to the vendor.
    pub fn offline_request(&self, info: &ClientInfo) -> String {
        let installation = self.lock().install.installation_id.clone();
        let body = json!({
            "v": 2,
            "installation_id": installation,
            "fingerprint": self.fingerprint,
            "computer_name": info.computer_name,
            "clinic_name": info.clinic_name,
            "app_version": info.app_version,
        });
        BASE64URL_NOPAD.encode(body.to_string().as_bytes())
    }

    /// Applies a token the vendor made for this computer (no internet needed).
    pub fn offline_apply(&self, response_code: &str) -> Result<LicenseStatus> {
        let token: String = response_code.chars().filter(|c| !c.is_whitespace()).collect();
        let installation = self.lock().install.installation_id.clone();
        let payload = verify_token(&self.keys(), &token)
            .ok_or_else(|| CoreError::api(ErrorCode::LicenseRejected, "response_code_invalid"))?;
        if payload.installation_id != installation || payload.fingerprint != self.fingerprint {
            return Err(CoreError::api(ErrorCode::LicenseRejected, "token_mismatch"));
        }
        write_atomic(&self.token_path(), &serde_json::to_vec_pretty(&json!({ "token": token }))?)?;
        self.reload();
        Ok(self.status())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ed25519_dalek::{Signer, SigningKey};
    use time::macros::datetime;

    use super::*;

    const FP: &str = "abababababababababababababababababababababababababababababababab";

    fn install(seen: &str) -> Install {
        Install {
            installation_id: "inst-1".into(),
            first_run_at: "2026-01-01T00:00:00.000Z".into(),
            last_seen_at: seen.into(),
        }
    }

    struct P {
        result: &'static str,
        kind: &'static str,
        issued: &'static str,
        expires: Option<&'static str>,
        valid_until: Option<&'static str>,
        check: i64,
        offline: i64,
        post: i64,
    }

    fn p() -> P {
        P {
            result: "valid",
            kind: "paid",
            issued: "2026-11-01T00:00:00.000Z",
            expires: Some("2027-11-01T00:00:00.000Z"),
            valid_until: Some("2027-03-01T00:00:00.000Z"),
            check: 30,
            offline: 120,
            post: 120,
        }
    }

    fn loaded(x: &P) -> Loaded {
        Loaded::Valid(Box::new(Payload {
            result: x.result.into(),
            kind: x.kind.into(),
            plan: "standard".into(),
            max_computers: 3,
            installation_id: "inst-1".into(),
            fingerprint: FP.into(),
            nonce: None,
            issued_at: x.issued.into(),
            expires_at: x.expires.map(Into::into),
            check_every_days: x.check,
            offline_grace_days: x.offline,
            post_expiry_grace_days: x.post,
            valid_until: x.valid_until.map(Into::into),
        }))
    }

    fn state(i: &Install, l: &Loaded, at: OffsetDateTime) -> LicenseStatus {
        evaluate(i, l, at, FP, true, false)
    }

    #[test]
    fn both_embedded_public_keys_are_valid_ed25519_keys_and_production_refuses_the_development_one() {
        for (kid, x, _) in PUBLIC_KEYS {
            let bytes: [u8; 32] = BASE64URL_NOPAD.decode(x.as_bytes()).unwrap().try_into().unwrap();
            assert!(VerifyingKey::from_bytes(&bytes).is_ok(), "{kid}");
        }
        assert_eq!(trusted_keys(true).len(), 2);
        let prod = trusted_keys(false);
        assert_eq!(prod.len(), 1);
        assert_eq!(prod[0].0, "cc-production-2026-10");
    }

    #[test]
    fn a_fresh_install_is_open_in_development_and_read_only_in_production() {
        let i = install("2026-11-01T00:00:00.000Z");
        let at = datetime!(2026-11-05 00:00 UTC);
        let dev = evaluate(&i, &Loaded::None, at, FP, true, true);
        assert_eq!((dev.state, dev.read_only), (LicenseState::Unactivated, false));
        let prod = evaluate(&i, &Loaded::None, at, FP, true, false);
        assert_eq!((prod.state, prod.read_only), (LicenseState::Unactivated, true));
        assert_eq!(prod.fingerprint_hint, "ABAB-ABAB");
    }

    #[test]
    fn a_paid_license_walks_through_active_check_due_expired_warning_and_read_only() {
        let i = install("2026-12-01T00:00:00.000Z");
        let l = loaded(&p()); // issued 11-01, check 30 d, valid until 2027-03-01, ends 2027-11-01
        let s = state(&i, &l, datetime!(2026-11-10 00:00 UTC));
        assert_eq!(
            (s.state, s.read_only, s.verify_due, s.computers),
            (LicenseState::Active, false, false, 3)
        );
        // 31 days since the last check: still works, with the reminder.
        let s = state(&i, &l, datetime!(2026-12-02 00:00 UTC));
        assert_eq!((s.state, s.read_only, s.verify_due), (LicenseState::Grace, false, true));
        assert_eq!(s.grace_days_left, Some(89));
        // Past valid_until without a check: read-only until the clinic is online again.
        let late = install("2027-03-02T00:00:00.000Z");
        let s = state(&late, &l, datetime!(2027-03-02 00:00 UTC));
        assert_eq!((s.state, s.read_only), (LicenseState::NeedsVerification, true));
    }

    #[test]
    fn after_the_end_date_there_is_a_warning_bar_for_the_vendors_grace_then_read_only() {
        let x = P {
            expires: Some("2026-12-01T00:00:00.000Z"),
            issued: "2026-12-10T00:00:00.000Z",
            valid_until: Some("2027-04-01T00:00:00.000Z"),
            ..p()
        };
        let l = loaded(&x);
        let i = install("2026-12-20T00:00:00.000Z");
        let s = state(&i, &l, datetime!(2026-12-20 00:00 UTC));
        assert_eq!((s.state, s.read_only, s.days_left), (LicenseState::ExpiredWarning, false, Some(-19)));
        assert_eq!(s.grace_days_left, Some(102));
        let i2 = install("2027-04-02T00:00:00.000Z");
        let s = state(&i2, &l, datetime!(2027-04-02 00:00 UTC));
        assert_eq!((s.state, s.read_only), (LicenseState::Expired, true));
    }

    #[test]
    fn a_trial_is_read_only_the_day_it_ends() {
        let x = P {
            kind: "trial",
            issued: "2026-11-01T00:00:00.000Z",
            expires: Some("2026-12-01T00:00:00.000Z"),
            valid_until: Some("2026-12-01T00:00:00.000Z"),
            check: 7,
            offline: 45,
            post: 0,
            ..p()
        };
        let l = loaded(&x);
        let i = install("2026-11-03T00:00:00.000Z");
        assert_eq!(state(&i, &l, datetime!(2026-11-03 00:00 UTC)).state, LicenseState::Trial);
        let i = install("2026-12-02T00:00:00.000Z");
        let s = state(&i, &l, datetime!(2026-12-02 00:00 UTC));
        assert_eq!((s.state, s.read_only), (LicenseState::Expired, true));
    }

    #[test]
    fn what_the_vendor_decided_arrives_as_the_result_of_the_token() {
        let i = install("2026-11-03T00:00:00.000Z");
        let at = datetime!(2026-11-03 00:00 UTC);
        for (result, want) in [
            ("pending", LicenseState::Pending),
            ("blocked", LicenseState::Blocked),
            ("revoked", LicenseState::Revoked),
            ("license_suspended", LicenseState::Revoked),
            ("license_expired", LicenseState::Expired),
            ("fingerprint_mismatch", LicenseState::Invalid),
            ("something_new", LicenseState::Invalid),
        ] {
            let s = state(&i, &loaded(&P { result, ..p() }), at);
            assert_eq!((s.state, s.read_only), (want, true), "{result}");
        }
    }

    #[test]
    fn another_computer_a_damaged_file_and_a_clock_set_back_are_read_only() {
        let i = install("2026-11-03T00:00:00.000Z");
        let at = datetime!(2026-11-03 00:00 UTC);
        let other_hw = evaluate(&i, &loaded(&p()), at, &"c".repeat(64), true, false);
        assert_eq!(other_hw.state, LicenseState::Invalid);
        assert_eq!(state(&i, &Loaded::Invalid, at).state, LicenseState::Invalid);
        let seen = install("2026-11-10T12:00:00.000Z");
        assert_eq!(
            state(&seen, &loaded(&p()), datetime!(2026-11-08 12:00 UTC)).state,
            LicenseState::ClockTampered
        );
        assert_eq!(state(&seen, &loaded(&p()), datetime!(2026-11-10 06:00 UTC)).state, LicenseState::Active);
    }

    // ── tokens and the manager, with a throw-away signing key and a scripted server ──

    fn make_token(key: &SigningKey, kid: &str, payload: &Value) -> String {
        let h =
            BASE64URL_NOPAD.encode(json!({"alg": "EdDSA", "kid": kid, "typ": "JWT"}).to_string().as_bytes());
        let p = BASE64URL_NOPAD.encode(payload.to_string().as_bytes());
        let sig = BASE64URL_NOPAD.encode(&key.sign(format!("{h}.{p}").as_bytes()).to_bytes());
        format!("{h}.{p}.{sig}")
    }

    #[test]
    fn tokens_verify_only_with_the_right_key_and_untouched() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let keys: Keys = vec![("k1".into(), sk.verifying_key().to_bytes())];
        let payload = json!({"result": "valid", "installation_id": "i", "issued_at": "2026-11-01T00:00:00Z"});
        let t = make_token(&sk, "k1", &payload);
        assert_eq!(verify_token(&keys, &t).unwrap().result, "valid");
        assert!(verify_token(&keys, &make_token(&sk, "other", &payload)).is_none(), "unknown kid");
        let forger = SigningKey::from_bytes(&[8u8; 32]);
        assert!(verify_token(&keys, &make_token(&forger, "k1", &payload)).is_none(), "wrong key");
        let parts: Vec<&str> = t.split('.').collect();
        let tampered_payload = BASE64URL_NOPAD.encode(
            json!({"result": "valid", "installation_id": "i2", "issued_at": "x"}).to_string().as_bytes(),
        );
        assert!(verify_token(&keys, &format!("{}.{}.{}", parts[0], tampered_payload, parts[2])).is_none());
        assert!(verify_token(&keys, "a.b").is_none() && verify_token(&keys, "").is_none());
        // alg must be EdDSA (an `alg: none` token is refused).
        let none_h = BASE64URL_NOPAD.encode(json!({"alg": "none", "kid": "k1"}).to_string().as_bytes());
        assert!(verify_token(&keys, &format!("{none_h}.{}.{}", parts[1], parts[2])).is_none());
    }

    struct Script {
        key: SigningKey,
        manager_installation: Mutex<String>,
        fingerprint: String,
        calls: Mutex<Vec<Value>>,
        /// `None`: answer like the server (token for the asked nonce); `Some(Err(code))`: refuse.
        refuse: Mutex<Option<String>>,
        result: Mutex<String>,
        wrong_nonce: Mutex<bool>,
    }

    struct Fake(Arc<Script>);

    impl LicenseServer for Fake {
        fn configured(&self) -> bool {
            true
        }
        fn validate(&self, body: &Value) -> std::result::Result<Value, ServerError> {
            let s = &self.0;
            s.calls.lock().unwrap().push(body.clone());
            if let Some(code) = s.refuse.lock().unwrap().clone() {
                return Err(ServerError::Rejected(code));
            }
            let nonce = if *s.wrong_nonce.lock().unwrap() {
                "0".repeat(32)
            } else {
                body["nonce"].as_str().unwrap().to_string()
            };
            let payload = json!({
                "result": *s.result.lock().unwrap(), "kind": "paid", "plan": "standard", "max_computers": 3,
                "installation_id": *s.manager_installation.lock().unwrap(), "fingerprint": s.fingerprint, "nonce": nonce,
                "issued_at": iso(now()), "expires_at": iso(now() + Duration::days(300)),
                "check_every_days": 30, "offline_grace_days": 120, "post_expiry_grace_days": 120,
                "valid_until": iso(now() + Duration::days(120)),
            });
            Ok(json!({"result": *s.result.lock().unwrap(), "token": make_token(&s.key, "t", &payload)}))
        }
    }

    const KEY: &str = "0A1B-2C3D-4E5F-6A7B-8C9D-0E1F";

    fn manager() -> (tempfile::TempDir, LicenseManager, Arc<Script>) {
        let dir = tempfile::tempdir().unwrap();
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let keys: Keys = vec![("t".into(), key.verifying_key().to_bytes())];
        let script = Arc::new(Script {
            key,
            manager_installation: Mutex::new(String::new()),
            fingerprint: FP.into(),
            calls: Mutex::new(Vec::new()),
            refuse: Mutex::new(None),
            result: Mutex::new("pending".into()),
            wrong_nonce: Mutex::new(false),
        });
        let m =
            LicenseManager::new(dir.path(), keys, Box::new(Fake(script.clone())), FP.into(), false).unwrap();
        *script.manager_installation.lock().unwrap() = m.lock().install.installation_id.clone();
        (dir, m, script)
    }

    fn info() -> ClientInfo {
        ClientInfo {
            clinic_name: "Clinic".into(),
            computer_name: "PC-1".into(),
            app_version: "0.11.0".into(),
            arch: "x64".into(),
            last_backup_at: None,
            last_backup_ok: None,
        }
    }

    #[test]
    fn entering_the_key_asks_the_server_then_waits_for_approval_and_checks_again_with_the_stored_key() {
        let (dir, m, script) = manager();
        let s = m.status();
        assert_eq!(
            (s.state, s.read_only),
            (LicenseState::Unactivated, true),
            "production rules: no key, no changes"
        );

        // New computer: the vendor has not approved it yet.
        let s = m.activate(&KEY.to_lowercase(), &info()).unwrap();
        assert_eq!((s.state, s.read_only), (LicenseState::Pending, true));
        let sent = script.calls.lock().unwrap()[0].clone();
        assert_eq!(sent["license_key"], KEY, "normalised");
        assert_eq!(sent["fingerprint"], FP);
        assert_eq!(sent["computer_name"], "PC-1");
        assert_eq!(sent["installation_id"], m.installation_id());
        assert_eq!(sent["nonce"].as_str().unwrap().len(), 32);

        // The vendor approves; the next check brings the working token. A restart keeps it.
        *script.result.lock().unwrap() = "valid".into();
        let s = m.verify(&info()).unwrap();
        assert_eq!(
            (s.state, s.read_only, s.plan.as_deref(), s.computers),
            (LicenseState::Active, false, Some("standard"), 3)
        );
        let again =
            LicenseManager::new(dir.path(), m.keys(), Box::new(Fake(script.clone())), FP.into(), false)
                .unwrap();
        assert_eq!(again.status().state, LicenseState::Active);
        assert_eq!(script.calls.lock().unwrap().len(), 2);
        assert_eq!(script.calls.lock().unwrap()[1]["license_key"], KEY, "the stored key is used");

        // The vendor blocks this computer: read-only at the next check.
        *script.result.lock().unwrap() = "blocked".into();
        let s = m.verify(&info()).unwrap();
        assert_eq!((s.state, s.read_only), (LicenseState::Blocked, true));
    }

    #[test]
    fn a_wrong_key_format_a_refusal_a_replayed_answer_and_a_missing_key_change_nothing() {
        let (dir, m, script) = manager();
        for bad in ["", "ARTA-7K2M-9QXP-4HTD", "0A1B-2C3D-4E5F-6A7B-8C9D-0E1G", "0A1B2C3D4E5F6A7B8C9D0E1F"] {
            let e = m.activate(bad, &info()).unwrap_err();
            assert_eq!(e.field(), Some("code"), "{bad}");
        }
        assert!(script.calls.lock().unwrap().is_empty(), "nothing is sent for a malformed key");

        *script.refuse.lock().unwrap() = Some("unknown_key".into());
        let e = m.activate(KEY, &info()).unwrap_err();
        assert_eq!(e.code(), ErrorCode::LicenseRejected);
        assert!(e.to_string().contains("unknown_key"));
        *script.refuse.lock().unwrap() = None;

        // An answer made for another request (an old token sent again) is refused.
        *script.wrong_nonce.lock().unwrap() = true;
        assert_eq!(m.activate(KEY, &info()).unwrap_err().code(), ErrorCode::LicenseRejected);
        *script.wrong_nonce.lock().unwrap() = false;
        assert!(!dir.path().join("license.json").exists());

        assert_eq!(
            m.verify(&info()).unwrap_err().code(),
            ErrorCode::LicenseRejected,
            "no key was ever accepted"
        );

        // This build has no server address.
        struct NoServer;
        impl LicenseServer for NoServer {
            fn configured(&self) -> bool {
                false
            }
            fn validate(&self, _: &Value) -> std::result::Result<Value, ServerError> {
                Err(ServerError::Unavailable("none".into()))
            }
        }
        let off = LicenseManager::new(dir.path(), m.keys(), Box::new(NoServer), FP.into(), false).unwrap();
        assert!(!off.status().server_configured);
        assert_eq!(off.activate(KEY, &info()).unwrap_err().code(), ErrorCode::LicenseServerUnavailable);
    }

    #[test]
    fn a_token_copied_to_another_computer_or_edited_by_hand_stops_counting() {
        let (dir, m, script) = manager();
        *script.result.lock().unwrap() = "valid".into();
        m.activate(KEY, &info()).unwrap();
        // Same token file, other hardware.
        let other =
            LicenseManager::new(dir.path(), m.keys(), Box::new(Fake(script.clone())), "c".repeat(64), false)
                .unwrap();
        assert_eq!(other.status().state, LicenseState::Invalid);
        // Edited file.
        let path = dir.path().join("license.json");
        let text = fs::read_to_string(&path).unwrap().replace('a', "b");
        fs::write(&path, text).unwrap();
        m.reload();
        assert_eq!(m.status().state, LicenseState::Invalid);
        fs::write(&path, b"not json").unwrap();
        m.reload();
        assert_eq!(m.status().state, LicenseState::Invalid);
    }

    #[test]
    fn offline_activation_round_trips_through_codes() {
        let (_dir, m, script) = manager();
        let request = m.offline_request(&info());
        let decoded: Value =
            serde_json::from_slice(&BASE64URL_NOPAD.decode(request.as_bytes()).unwrap()).unwrap();
        assert_eq!(
            (decoded["installation_id"].as_str(), decoded["fingerprint"].as_str()),
            (Some(m.installation_id().as_str()), Some(FP))
        );

        // The vendor makes a token for exactly this computer (no nonce: it was not asked online).
        let payload = json!({
            "result": "valid", "kind": "paid", "plan": "pro", "max_computers": 6, "installation_id": m.installation_id(), "fingerprint": FP,
            "issued_at": iso(now()), "expires_at": iso(now() + Duration::days(365)), "valid_until": iso(now() + Duration::days(120)),
        });
        let token = make_token(&script.key, "t", &payload);
        let s = m.offline_apply(&format!("{}\n{}", &token[..40], &token[40..])).unwrap();
        assert_eq!((s.state, s.plan.as_deref()), (LicenseState::Active, Some("pro")));

        let wrong = json!({"result": "valid", "installation_id": "someone-else", "fingerprint": FP, "issued_at": iso(now())});
        assert_eq!(
            m.offline_apply(&make_token(&script.key, "t", &wrong)).unwrap_err().code(),
            ErrorCode::LicenseRejected
        );
        assert_eq!(m.offline_apply("not a token").unwrap_err().code(), ErrorCode::LicenseRejected);
    }

    #[test]
    fn the_clock_mark_moves_forward_only_and_a_set_back_clock_is_read_only() {
        let (_dir, m, _script) = manager();
        let t0 = now();
        assert_eq!(m.status_at(t0 + Duration::days(3)).state, LicenseState::Unactivated);
        let s = m.status_at(t0);
        assert_eq!((s.state, s.read_only), (LicenseState::ClockTampered, true));
        assert_eq!(m.status_at(t0 + Duration::days(4)).state, LicenseState::Unactivated);
    }

    #[test]
    fn the_background_check_needs_a_key_a_due_date_and_waits_an_hour_between_tries() {
        let (_dir, m, script) = manager();
        assert!(!m.should_check_now(), "no key yet");
        *script.result.lock().unwrap() = "valid".into();
        m.activate(KEY, &info()).unwrap();
        assert!(!m.should_check_now(), "just checked");
    }
}
