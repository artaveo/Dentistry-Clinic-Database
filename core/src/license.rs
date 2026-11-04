//! License, trial and read-only state (roadmap 10.8-10.9, ADR-15, ADR-16).
//!
//! * The license is a file signed with Ed25519 by the central server; the app holds only the public
//!   key (`license_public_key.txt`), so a license cannot be forged on the clinic's computer.
//! * It lives in `license.json` and `install.json` *beside* the database, never inside it: restoring an
//!   old backup can not bring back an expired trial or a revoked license.
//! * Every answer of the server is a freshly signed license with a new `verified_at`. The app asks
//!   about every 7 days when it is online; if the last answer is older than the grace period (30
//!   days, set by the server) the app becomes read-only until it can ask again.
//! * Read-only (10.9) refuses changes but never data access: viewing, printing, export, backup and
//!   restore keep working.
//! * A clock set back is noticed (`install.json` remembers the latest time seen) and also means read-only.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration as StdDuration;

use artaveo_shared::{ErrorCode, LicenseState, LicenseStatus};
use data_encoding::{BASE64, BASE64URL_NOPAD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::{Duration, OffsetDateTime};

use crate::clock::{iso, now, parse};
use crate::error::{CoreError, Result};
use crate::ids::new_id;

/// Public half of the signing key (the private half is only a secret of the Supabase project).
pub const PUBLIC_KEY_B64: &str = include_str!("../license_public_key.txt");

/// A fresh install works normally for this long without being activated (nothing is forced on a
/// clinic that has no internet on the day of installation).
pub const FIRST_PERIOD_DAYS: i64 = 14;
/// A clock this far behind the latest time seen counts as tampering.
const CLOCK_SLACK_HOURS: i64 = 24;
/// `install.json` is rewritten at most this often when only the "latest time seen" moved.
const SEEN_WRITE_EVERY_MINUTES: i64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LicenseFile {
    /// The signed text (JSON), kept exactly as the server wrote it.
    pub payload: String,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Payload {
    pub v: u32,
    pub license_id: String,
    pub clinic_id: String,
    pub installation_id: String,
    /// "trial" | "paid"
    pub kind: String,
    /// "trial" | "basic" | "standard" | "pro"
    pub plan: String,
    pub computers: u32,
    pub issued_at: String,
    pub verified_at: String,
    pub starts_at: String,
    pub expires_at: String,
    pub offline_grace_days: i64,
    pub verify_every_days: i64,
}

/// What this computer remembers about itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Install {
    pub installation_id: String,
    pub first_run_at: String,
    /// The latest time ever seen on this computer's clock (clock-tamper check).
    pub last_seen_at: String,
    /// The server said this installation or license is revoked.
    #[serde(default)]
    pub revoked: bool,
}

/// The license file as far as it could be read.
#[derive(Debug, Clone)]
pub enum Loaded {
    None,
    /// Signature wrong, file damaged, or made for another computer.
    Invalid,
    Valid(Box<Payload>),
}

pub fn verify_file(public_key_b64: &str, file: &LicenseFile) -> Option<Payload> {
    let key_bytes: [u8; 32] = BASE64.decode(public_key_b64.trim().as_bytes()).ok()?.try_into().ok()?;
    let key = VerifyingKey::from_bytes(&key_bytes).ok()?;
    let sig = Signature::from_slice(&BASE64.decode(file.signature.trim().as_bytes()).ok()?).ok()?;
    key.verify(file.payload.as_bytes(), &sig).ok()?;
    let payload: Payload = serde_json::from_str(&file.payload).ok()?;
    (payload.v == 1).then_some(payload)
}

fn days_between(later: OffsetDateTime, earlier: OffsetDateTime) -> i64 {
    (later - earlier).whole_days()
}

/// The pure rule: what state is this computer in right now? (Everything the tests need.)
pub fn evaluate(
    install: &Install,
    loaded: &Loaded,
    at: OffsetDateTime,
    installation_id: &str,
    server_configured: bool,
) -> LicenseStatus {
    let mut s = LicenseStatus {
        state: LicenseState::Active,
        read_only: false,
        kind: None,
        plan: None,
        computers: 6,
        installation_id: installation_id.to_string(),
        expires_at: None,
        days_left: None,
        verified_at: None,
        verify_due: false,
        grace_days_left: None,
        first_period_days_left: None,
        server_configured,
    };
    let stop = |mut s: LicenseStatus, state: LicenseState| {
        s.state = state;
        s.read_only = true;
        s
    };
    if let Loaded::Valid(p) = loaded {
        s.kind = Some(p.kind.clone());
        s.plan = Some(p.plan.clone());
        s.computers = p.computers;
        s.expires_at = Some(p.expires_at.clone());
        s.verified_at = Some(p.verified_at.clone());
    }
    if install.revoked {
        return stop(s, LicenseState::Revoked);
    }
    if parse(&install.last_seen_at).is_some_and(|seen| at < seen - Duration::hours(CLOCK_SLACK_HOURS)) {
        return stop(s, LicenseState::ClockTampered);
    }
    let payload = match loaded {
        Loaded::None => {
            let first = parse(&install.first_run_at).unwrap_or(at);
            let left = FIRST_PERIOD_DAYS - days_between(at, first);
            s.state = LicenseState::Unactivated;
            s.first_period_days_left = Some(left.max(0));
            s.read_only = left <= 0;
            return s;
        }
        Loaded::Invalid => return stop(s, LicenseState::Invalid),
        Loaded::Valid(p) => p,
    };
    if payload.installation_id != install.installation_id {
        return stop(s, LicenseState::Invalid);
    }
    let (Some(expires), Some(verified)) = (parse(&payload.expires_at), parse(&payload.verified_at)) else {
        return stop(s, LicenseState::Invalid);
    };
    // Whole days left, counting a part of a day as a day.
    s.days_left = Some(((expires - at).whole_seconds() as f64 / 86_400.0).ceil() as i64);
    if at > expires {
        return stop(s, LicenseState::Expired);
    }
    let since_check = days_between(at, verified);
    if since_check > payload.offline_grace_days {
        s.verify_due = true;
        return stop(s, LicenseState::NeedsVerification);
    }
    s.verify_due = since_check >= payload.verify_every_days;
    if s.verify_due {
        s.state = LicenseState::Grace;
        s.grace_days_left = Some((payload.offline_grace_days - since_check).max(0));
    } else {
        s.state = if payload.kind == "trial" { LicenseState::Trial } else { LicenseState::Active };
    }
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
    /// The server answered "no", with its reason code (`code_unknown`, `license_revoked`, ...).
    Rejected(String),
}

pub trait LicenseServer: Send + Sync {
    fn configured(&self) -> bool;
    fn post(&self, endpoint: &str, body: &Value) -> std::result::Result<Value, ServerError>;
}

/// The real server: Supabase Edge Functions over HTTPS. The address and the public (publishable) key
/// are given when the app is built (`ARTAVEO_LICENSE_URL`, `ARTAVEO_LICENSE_ANON_KEY`); no secret is
/// in the program (roadmap 10.10).
pub struct HttpServer {
    base: Option<String>,
    key: Option<String>,
}

impl HttpServer {
    pub fn from_build() -> Self {
        let pick = |name: &str, built: Option<&str>| {
            std::env::var(name).ok().or_else(|| built.map(str::to_string)).filter(|v| !v.trim().is_empty())
        };
        Self {
            base: pick("ARTAVEO_LICENSE_URL", option_env!("ARTAVEO_LICENSE_URL")),
            key: pick("ARTAVEO_LICENSE_ANON_KEY", option_env!("ARTAVEO_LICENSE_ANON_KEY")),
        }
    }
}

impl LicenseServer for HttpServer {
    fn configured(&self) -> bool {
        self.base.is_some()
    }

    fn post(&self, endpoint: &str, body: &Value) -> std::result::Result<Value, ServerError> {
        let Some(base) = &self.base else { return Err(ServerError::Unavailable("no server address".into())) };
        let tls = native_tls::TlsConnector::new().map_err(|e| ServerError::Unavailable(e.to_string()))?;
        let agent = ureq::AgentBuilder::new()
            .tls_connector(std::sync::Arc::new(tls))
            .timeout(StdDuration::from_secs(15))
            .build();
        let mut req = agent
            .post(&format!("{}/{endpoint}", base.trim_end_matches('/')))
            .set("content-type", "application/json");
        if let Some(key) = &self.key {
            req = req.set("apikey", key).set("authorization", &format!("Bearer {key}"));
        }
        match req.send_string(&body.to_string()) {
            Ok(resp) => resp
                .into_string()
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .ok_or_else(|| ServerError::Unavailable("unreadable answer".into())),
            Err(ureq::Error::Status(_, resp)) => {
                let reason = resp
                    .into_string()
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_else(|| "server_error".into());
                if reason == "server_error" {
                    Err(ServerError::Unavailable(reason))
                } else {
                    Err(ServerError::Rejected(reason))
                }
            }
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
    public_key: Mutex<String>,
    server: Mutex<Box<dyn LicenseServer>>,
    state: Mutex<State>,
    /// Last time the background check tried to reach the server.
    last_try: Mutex<Option<OffsetDateTime>>,
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

impl LicenseManager {
    pub fn new(dir: &Path, public_key_b64: &str, server: Box<dyn LicenseServer>) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let at = now();
        let install_path = dir.join("install.json");
        let install =
            match fs::read(&install_path).ok().and_then(|b| serde_json::from_slice::<Install>(&b).ok()) {
                Some(i) => i,
                None => {
                    let i = Install {
                        installation_id: new_id(),
                        first_run_at: iso(at),
                        last_seen_at: iso(at),
                        revoked: false,
                    };
                    write_atomic(&install_path, &serde_json::to_vec_pretty(&i)?)?;
                    i
                }
            };
        let m = Self {
            dir: dir.to_path_buf(),
            public_key: Mutex::new(public_key_b64.to_string()),
            server: Mutex::new(server),
            state: Mutex::new(State { install, loaded: Loaded::None, written_at: at }),
            last_try: Mutex::new(None),
        };
        m.reload();
        Ok(m)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn file_path(&self) -> PathBuf {
        self.dir.join("license.json")
    }

    /// Re-reads the license file (after it was written, or by a test).
    fn reload(&self) {
        let key = self.public_key.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let mut st = self.lock();
        st.loaded = match fs::read(self.file_path()) {
            Err(_) => Loaded::None,
            Ok(bytes) => match serde_json::from_slice::<LicenseFile>(&bytes) {
                Err(_) => Loaded::Invalid,
                Ok(file) => match verify_file(&key, &file) {
                    Some(p) if p.installation_id == st.install.installation_id => Loaded::Valid(Box::new(p)),
                    _ => Loaded::Invalid,
                },
            },
        };
    }

    #[cfg(test)]
    pub fn set_server(&self, server: Box<dyn LicenseServer>) {
        *self.server.lock().unwrap() = server;
    }

    #[cfg(test)]
    pub fn set_public_key(&self, key: &str) {
        *self.public_key.lock().unwrap() = key.to_string();
        self.reload();
    }

    #[cfg(test)]
    pub fn installation_id(&self) -> String {
        self.lock().install.installation_id.clone()
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
        let result = evaluate(&st.install, &st.loaded, at, &st.install.installation_id, configured);
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
        if !s.server_configured || !s.verify_due || s.state == LicenseState::Revoked {
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

    fn call(&self, endpoint: &str, body: Value) -> Result<Value> {
        let server = self.server.lock().unwrap_or_else(|p| p.into_inner());
        server.post(endpoint, &body).map_err(|e| match e {
            ServerError::Unavailable(d) => CoreError::api(ErrorCode::LicenseServerUnavailable, d),
            ServerError::Rejected(code) => CoreError::api(ErrorCode::LicenseRejected, code),
        })
    }

    fn base_body(&self, info: &ClientInfo) -> Value {
        json!({
            "installation_id": self.lock().install.installation_id,
            "clinic_name": info.clinic_name,
            "computer_name": info.computer_name,
            "app_version": info.app_version,
            "arch": info.arch,
        })
    }

    /// Checks a license the server (or an offline response code) gave us and keeps it.
    fn accept(&self, file: &LicenseFile) -> Result<()> {
        let key = self.public_key.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let id = self.lock().install.installation_id.clone();
        let ok = verify_file(&key, file).is_some_and(|p| p.installation_id == id);
        if !ok {
            return Err(CoreError::api(ErrorCode::LicenseRejected, "license_invalid"));
        }
        write_atomic(&self.file_path(), &serde_json::to_vec_pretty(file)?)?;
        {
            let mut st = self.lock();
            // A fresh valid license from the server lifts an earlier "revoked" mark (reinstated by the vendor).
            if st.install.revoked {
                st.install.revoked = false;
                self.save_install(&mut st)?;
            }
        }
        self.reload();
        Ok(())
    }

    fn accept_answer(&self, answer: &Value) -> Result<()> {
        if answer.get("revoked").and_then(Value::as_bool) == Some(true) {
            let mut st = self.lock();
            st.install.revoked = true;
            self.save_install(&mut st)?;
            return Ok(());
        }
        let file: LicenseFile =
            serde_json::from_value(answer.get("license").cloned().ok_or_else(|| {
                CoreError::api(ErrorCode::LicenseServerUnavailable, "no license in answer")
            })?)
            .map_err(|_| CoreError::api(ErrorCode::LicenseServerUnavailable, "unreadable license"))?;
        self.accept(&file)
    }

    /// Asks the server for the one-time 30-day trial of this installation (10.8).
    pub fn start_trial(&self, info: &ClientInfo) -> Result<LicenseStatus> {
        let answer = self.call("license-trial", self.base_body(info))?;
        self.accept_answer(&answer)?;
        Ok(self.status())
    }

    pub fn activate(&self, code: &str, info: &ClientInfo) -> Result<LicenseStatus> {
        let mut body = self.base_body(info);
        body["code"] = json!(code.trim());
        let answer = self.call("license-activate", body)?;
        self.accept_answer(&answer)?;
        Ok(self.status())
    }

    /// The periodic check (about every 7 days while online).
    pub fn verify(&self, info: &ClientInfo) -> Result<LicenseStatus> {
        let body = json!({
            "installation_id": self.lock().install.installation_id,
            "app_version": info.app_version,
            "last_backup_at": info.last_backup_at,
            "last_backup_ok": info.last_backup_ok,
        });
        let answer = self.call("license-verify", body)?;
        self.accept_answer(&answer)?;
        Ok(self.status())
    }

    /// Gives this computer's place back so another computer can activate (transfer, 10.8).
    pub fn release(&self) -> Result<LicenseStatus> {
        let id = self.lock().install.installation_id.clone();
        self.call("license-release", json!({ "installation_id": id }))?;
        let _ = fs::remove_file(self.file_path());
        self.reload();
        Ok(self.status())
    }

    /// The short text a clinic without internet sends to the vendor.
    pub fn offline_request(&self, info: &ClientInfo) -> String {
        let body = json!({
            "v": 1,
            "installation_id": self.lock().install.installation_id,
            "clinic_name": info.clinic_name,
            "computer_name": info.computer_name,
            "app_version": info.app_version,
        });
        BASE64URL_NOPAD.encode(body.to_string().as_bytes())
    }

    /// Applies the vendor's answer to an offline request: the signed license itself, as text.
    pub fn offline_apply(&self, response_code: &str) -> Result<LicenseStatus> {
        let cleaned: String = response_code.chars().filter(|c| !c.is_whitespace()).collect();
        let bytes = BASE64
            .decode(cleaned.as_bytes())
            .map_err(|_| CoreError::api(ErrorCode::LicenseRejected, "response_code_invalid"))?;
        let file: LicenseFile = serde_json::from_slice(&bytes)
            .map_err(|_| CoreError::api(ErrorCode::LicenseRejected, "response_code_invalid"))?;
        self.accept(&file)?;
        Ok(self.status())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ed25519_dalek::{Signer, SigningKey};
    use time::macros::datetime;

    use super::*;

    fn payload(installation: &str, kind: &str, verified: &str, expires: &str) -> Box<Payload> {
        Box::new(Payload {
            v: 1,
            license_id: "l".into(),
            clinic_id: "c".into(),
            installation_id: installation.into(),
            kind: kind.into(),
            plan: if kind == "trial" { "trial".into() } else { "standard".into() },
            computers: 3,
            issued_at: verified.into(),
            verified_at: verified.into(),
            starts_at: "2026-01-01T00:00:00.000Z".into(),
            expires_at: expires.into(),
            offline_grace_days: 30,
            verify_every_days: 7,
        })
    }

    fn install(first: &str, seen: &str) -> Install {
        Install {
            installation_id: "inst-1".into(),
            first_run_at: first.into(),
            last_seen_at: seen.into(),
            revoked: false,
        }
    }

    fn state(i: &Install, l: &Loaded, at: OffsetDateTime) -> LicenseStatus {
        evaluate(i, l, at, "inst-1", true)
    }

    #[test]
    fn the_server_test_vector_verifies_in_rust() {
        // Made by server/scripts/make-test-vector.mjs with the same code the Edge Functions use.
        let v: Value = serde_json::from_str(include_str!("../../server/test-vectors/license.json")).unwrap();
        let key = v["public_key"].as_str().unwrap();
        let valid: LicenseFile = serde_json::from_value(v["valid"].clone()).unwrap();
        let tampered: LicenseFile = serde_json::from_value(v["tampered"].clone()).unwrap();
        let p = verify_file(key, &valid).expect("signed by the server code");
        assert_eq!(
            (p.plan.as_str(), p.computers, p.installation_id.as_str()),
            ("standard", 3, "inst-test-1")
        );
        assert!(verify_file(key, &tampered).is_none(), "a changed payload must not verify");
        assert!(verify_file(PUBLIC_KEY_B64, &valid).is_none(), "another key must not verify");
    }

    #[test]
    fn the_shipped_public_key_is_a_valid_ed25519_key() {
        let bytes: [u8; 32] = BASE64.decode(PUBLIC_KEY_B64.trim().as_bytes()).unwrap().try_into().unwrap();
        assert!(VerifyingKey::from_bytes(&bytes).is_ok());
    }

    #[test]
    fn a_new_install_works_for_the_first_period_then_is_read_only() {
        let i = install("2026-11-01T00:00:00.000Z", "2026-11-01T00:00:00.000Z");
        let s = state(&i, &Loaded::None, datetime!(2026-11-05 00:00 UTC));
        assert_eq!(
            (s.state, s.read_only, s.first_period_days_left),
            (LicenseState::Unactivated, false, Some(10))
        );
        let s = state(&i, &Loaded::None, datetime!(2026-11-20 00:00 UTC));
        assert_eq!(
            (s.state, s.read_only, s.first_period_days_left),
            (LicenseState::Unactivated, true, Some(0))
        );
    }

    #[test]
    fn paid_license_walks_through_active_grace_needs_verification_and_expired() {
        let i = install("2026-01-01T00:00:00.000Z", "2026-11-01T00:00:00.000Z");
        let l =
            Loaded::Valid(payload("inst-1", "paid", "2026-11-01T00:00:00.000Z", "2027-01-01T00:00:00.000Z"));
        let s = state(&i, &l, datetime!(2026-11-03 00:00 UTC));
        assert_eq!((s.state, s.read_only, s.verify_due), (LicenseState::Active, false, false));
        assert_eq!(s.days_left, Some(59));
        assert_eq!(s.plan.as_deref(), Some("standard"));
        let s = state(&i, &l, datetime!(2026-11-10 00:00 UTC));
        assert_eq!(
            (s.state, s.read_only, s.verify_due, s.grace_days_left),
            (LicenseState::Grace, false, true, Some(21))
        );
        let s = state(&i, &l, datetime!(2026-12-05 00:00 UTC));
        assert_eq!((s.state, s.read_only), (LicenseState::NeedsVerification, true));
        // Verified recently but past the end date.
        let l =
            Loaded::Valid(payload("inst-1", "paid", "2027-01-02T00:00:00.000Z", "2027-01-01T00:00:00.000Z"));
        let i2 = install("2026-01-01T00:00:00.000Z", "2027-01-02T00:00:00.000Z");
        let s = state(&i2, &l, datetime!(2027-01-03 00:00 UTC));
        assert_eq!((s.state, s.read_only), (LicenseState::Expired, true));
    }

    #[test]
    fn trial_revoked_other_computer_damaged_and_clock_tampering() {
        let i = install("2026-11-01T00:00:00.000Z", "2026-11-01T00:00:00.000Z");
        let trial =
            Loaded::Valid(payload("inst-1", "trial", "2026-11-01T00:00:00.000Z", "2026-12-01T00:00:00.000Z"));
        assert_eq!(state(&i, &trial, datetime!(2026-11-02 00:00 UTC)).state, LicenseState::Trial);
        assert_eq!(state(&i, &trial, datetime!(2026-12-02 00:00 UTC)).state, LicenseState::Expired);

        let revoked = Install { revoked: true, ..i.clone() };
        let s = state(&revoked, &trial, datetime!(2026-11-02 00:00 UTC));
        assert_eq!((s.state, s.read_only), (LicenseState::Revoked, true));

        let other = Loaded::Valid(payload(
            "inst-OTHER",
            "paid",
            "2026-11-01T00:00:00.000Z",
            "2027-11-01T00:00:00.000Z",
        ));
        assert_eq!(state(&i, &other, datetime!(2026-11-02 00:00 UTC)).state, LicenseState::Invalid);
        assert_eq!(state(&i, &Loaded::Invalid, datetime!(2026-11-02 00:00 UTC)).state, LicenseState::Invalid);

        // The clock was put back two days: read-only; a few hours is only drift.
        let seen = install("2026-01-01T00:00:00.000Z", "2026-11-10T12:00:00.000Z");
        let l =
            Loaded::Valid(payload("inst-1", "paid", "2026-11-09T00:00:00.000Z", "2027-11-01T00:00:00.000Z"));
        assert_eq!(state(&seen, &l, datetime!(2026-11-08 12:00 UTC)).state, LicenseState::ClockTampered);
        assert_eq!(state(&seen, &l, datetime!(2026-11-10 06:00 UTC)).state, LicenseState::Active);
    }

    // ── the manager with a scripted server and a test key pair ──

    struct Script {
        key: SigningKey,
        installation: Mutex<String>,
        calls: Mutex<Vec<String>>,
        reply: Mutex<Option<std::result::Result<Value, String>>>,
        verified_at: Mutex<String>,
        expires_at: Mutex<String>,
        kind: Mutex<String>,
    }

    impl Script {
        fn license(&self) -> Value {
            let p = json!({
                "v": 1, "license_id": "lic-1", "clinic_id": "clinic-1",
                "installation_id": *self.installation.lock().unwrap(), "kind": *self.kind.lock().unwrap(),
                "plan": "standard", "computers": 3,
                "issued_at": *self.verified_at.lock().unwrap(), "verified_at": *self.verified_at.lock().unwrap(),
                "starts_at": "2026-01-01T00:00:00.000Z", "expires_at": *self.expires_at.lock().unwrap(),
                "offline_grace_days": 30, "verify_every_days": 7
            })
            .to_string();
            let sig = BASE64.encode(&self.key.sign(p.as_bytes()).to_bytes());
            json!({ "payload": p, "signature": sig })
        }
    }

    struct Fake(Arc<Script>);

    impl LicenseServer for Fake {
        fn configured(&self) -> bool {
            true
        }
        fn post(&self, endpoint: &str, _body: &Value) -> std::result::Result<Value, ServerError> {
            self.0.calls.lock().unwrap().push(endpoint.to_string());
            match self.0.reply.lock().unwrap().clone() {
                Some(Err(code)) => Err(ServerError::Rejected(code)),
                Some(Ok(v)) => Ok(v),
                None => Ok(json!({ "license": self.0.license() })),
            }
        }
    }

    fn manager() -> (tempfile::TempDir, LicenseManager, Arc<Script>) {
        let dir = tempfile::tempdir().unwrap();
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let public = BASE64.encode(key.verifying_key().as_bytes());
        let script = Arc::new(Script {
            key,
            installation: Mutex::new(String::new()),
            calls: Mutex::new(Vec::new()),
            reply: Mutex::new(None),
            verified_at: Mutex::new(iso(now())),
            expires_at: Mutex::new(iso(now() + Duration::days(365))),
            kind: Mutex::new("paid".into()),
        });
        let m = LicenseManager::new(dir.path(), &public, Box::new(Fake(script.clone()))).unwrap();
        *script.installation.lock().unwrap() = m.lock().install.installation_id.clone();
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
    fn activation_verification_and_release_follow_the_servers_answers() {
        let (dir, m, script) = manager();
        assert_eq!(m.status().state, LicenseState::Unactivated);

        let s = m.activate("ARTA-7K2M-9QXP-4HTD", &info()).unwrap();
        assert_eq!((s.state, s.plan.as_deref(), s.computers), (LicenseState::Active, Some("standard"), 3));
        assert!(dir.path().join("license.json").exists());

        // A new manager on the same folder finds the license again (restart).
        let again = LicenseManager::new(
            dir.path(),
            &BASE64.encode(script.key.verifying_key().as_bytes()),
            Box::new(Fake(script.clone())),
        )
        .unwrap();
        assert_eq!(again.status().state, LicenseState::Active);

        // The vendor revokes: the next check flags it, read-only follows.
        *script.reply.lock().unwrap() = Some(Ok(json!({"revoked": true, "reason": "license_revoked"})));
        let s = m.verify(&info()).unwrap();
        assert_eq!((s.state, s.read_only), (LicenseState::Revoked, true));
        // Reinstated: a fresh valid license lifts the mark.
        *script.reply.lock().unwrap() = None;
        let s = m.verify(&info()).unwrap();
        assert_eq!((s.state, s.read_only), (LicenseState::Active, false));

        let s = m.release().unwrap();
        assert_eq!(s.state, LicenseState::Unactivated);
        assert!(!dir.path().join("license.json").exists());
        assert_eq!(
            script.calls.lock().unwrap().as_slice(),
            ["license-activate", "license-verify", "license-verify", "license-release"]
        );
    }

    #[test]
    fn the_server_saying_no_or_being_away_is_reported_and_changes_nothing() {
        let (dir, m, script) = manager();
        *script.reply.lock().unwrap() = Some(Err("code_unknown".into()));
        let e = m.activate("ARTA-AAAA-BBBB-CCCC", &info()).unwrap_err();
        assert_eq!(e.code(), ErrorCode::LicenseRejected);
        assert!(e.to_string().contains("code_unknown"));
        assert!(!dir.path().join("license.json").exists());

        // A license for another installation, or signed with another key, is refused.
        *script.reply.lock().unwrap() = None;
        *script.installation.lock().unwrap() = "someone-else".into();
        assert_eq!(
            m.activate("ARTA-AAAA-BBBB-CCCC", &info()).unwrap_err().code(),
            ErrorCode::LicenseRejected
        );
        *script.installation.lock().unwrap() = m.installation_id();
        m.set_public_key(&BASE64.encode(SigningKey::from_bytes(&[9u8; 32]).verifying_key().as_bytes()));
        assert_eq!(
            m.activate("ARTA-AAAA-BBBB-CCCC", &info()).unwrap_err().code(),
            ErrorCode::LicenseRejected
        );

        // No server address in this build.
        let unconfigured =
            LicenseManager::new(dir.path(), "x", Box::new(HttpServer { base: None, key: None })).unwrap();
        assert!(!unconfigured.status().server_configured);
        assert_eq!(
            unconfigured.start_trial(&info()).unwrap_err().code(),
            ErrorCode::LicenseServerUnavailable
        );
    }

    #[test]
    fn a_license_file_edited_by_hand_stops_counting() {
        let (dir, m, _script) = manager();
        m.activate("ARTA-7K2M-9QXP-4HTD", &info()).unwrap();
        let path = dir.path().join("license.json");
        let text = fs::read_to_string(&path).unwrap().replace("standard", "pro");
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
        assert_eq!(decoded["installation_id"], m.installation_id());
        assert_eq!(decoded["clinic_name"], "Clinic");

        // The vendor's panel answers with base64 of the signed license.
        let response = BASE64.encode(script.license().to_string().as_bytes());
        let s = m.offline_apply(&format!("{}\n{}", &response[..40], &response[40..])).unwrap();
        assert_eq!(s.state, LicenseState::Active);
        assert_eq!(m.offline_apply("not base64 !!").unwrap_err().code(), ErrorCode::LicenseRejected);
    }

    #[test]
    fn the_clock_mark_moves_forward_only_and_a_set_back_clock_is_read_only() {
        let (_dir, m, _script) = manager();
        let t0 = now();
        assert_eq!(m.status_at(t0 + Duration::days(3)).state, LicenseState::Unactivated);
        // Three days later was seen; now the clock shows today again: more than a day behind.
        let s = m.status_at(t0);
        assert_eq!((s.state, s.read_only), (LicenseState::ClockTampered, true));
        assert_eq!(m.status_at(t0 + Duration::days(4)).state, LicenseState::Unactivated);
    }
}
