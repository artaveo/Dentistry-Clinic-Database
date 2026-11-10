//! LAN mode (roadmap Phase 7): the Core's side of the multi-computer setup.
//!
//! The network transport (TLS, WebSocket, mDNS, SPAKE2 pairing) lives in the `artaveo-lan` crate;
//! everything that needs the database or the permission model lives here, so a paired computer
//! can never do more than a person at the server could (ADR-01/02):
//!
//! * the **hub**: pairing window (6-digit code), live events for every connected screen, the
//!   list of connected computers and "who has this record open" (7.3);
//! * the **device register** in the encrypted database (7.4) with the client limit of the plan;
//! * the RPC methods `lan.*` and `presence.*`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use artaveo_shared::{
    methods as m, ErrorCode, LanDevice, LanPairingInfo, LanStatus, LiveEvent, PresenceParams, PresenceViewer,
    ValidationRule,
};
use rand::Rng;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;

use crate::audit::{self, Actor};
use crate::auth::perm;
use crate::clock::now_iso;
use crate::core::Core;
use crate::error::{CoreError, Result};
use crate::ids::new_id;
use crate::session::Session;

/// Computers (besides the server) a clinic may pair until the License (Phase 10) says otherwise:
/// the largest plan has 6 computers, one of which is the server.
pub const DEFAULT_CLIENT_LIMIT: u32 = 5;
pub const MAX_PAIRING_ATTEMPTS: u32 = 5;
const PAIRING_TTL: Duration = Duration::from_secs(10 * 60);
/// A screen that stops reporting for this long no longer counts as "has this record open".
const PRESENCE_TTL: Duration = Duration::from_secs(150);

struct PairingWindow {
    code: String,
    attempts_left: u32,
    issued: Instant,
}

/// What the transport reports about itself (shown on the Network page).
#[derive(Debug, Clone, Default)]
pub struct Runtime {
    pub running: bool,
    pub port: Option<u16>,
    pub addresses: Vec<String>,
}

struct Viewer {
    user_id: String,
    user_name: String,
    device: String,
    seen: Instant,
}

/// State shared by every transport; one per [`Core`].
pub struct LanHub {
    events: broadcast::Sender<LiveEvent>,
    pairing: Mutex<Option<PairingWindow>>,
    runtime: Mutex<Runtime>,
    connected: Mutex<HashMap<String, u32>>,
    /// `(entity, id)` → who has it open.
    presence: Mutex<HashMap<(String, String), Vec<Viewer>>>,
}

impl Default for LanHub {
    fn default() -> Self {
        Self::new()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl LanHub {
    pub fn new() -> Self {
        Self {
            events: broadcast::channel(512).0,
            pairing: Mutex::new(None),
            runtime: Mutex::new(Runtime::default()),
            connected: Mutex::new(HashMap::new()),
            presence: Mutex::new(HashMap::new()),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.events.subscribe()
    }

    pub fn publish(&self, topic: &str, method: &str) {
        // No receiver (nobody connected) is fine.
        let _ = self.events.send(LiveEvent { topic: topic.into(), method: method.into() });
    }

    // ── pairing window (7.2) ──

    /// Opens a fresh 6-digit pairing window (an older one is replaced).
    pub fn open_pairing(&self) -> LanPairingInfo {
        let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000));
        *lock(&self.pairing) =
            Some(PairingWindow { code, attempts_left: MAX_PAIRING_ATTEMPTS, issued: Instant::now() });
        self.pairing_info().expect("just opened")
    }

    pub fn cancel_pairing(&self) {
        *lock(&self.pairing) = None;
    }

    pub fn pairing_info(&self) -> Option<LanPairingInfo> {
        let mut w = lock(&self.pairing);
        match w.as_ref() {
            Some(p) if p.issued.elapsed() < PAIRING_TTL && p.attempts_left > 0 => Some(LanPairingInfo {
                code: p.code.clone(),
                expires_in_seconds: (PAIRING_TTL - p.issued.elapsed()).as_secs() as u32,
                attempts_left: p.attempts_left,
            }),
            Some(_) => {
                *w = None;
                None
            }
            None => None,
        }
    }

    /// The code a pairing attempt is checked against, while the window is open.
    pub fn pairing_code(&self) -> Option<String> {
        self.pairing_info().map(|p| p.code)
    }

    /// A wrong code: one attempt less; the window closes at zero. Returns the attempts left.
    pub fn pairing_failed(&self) -> u32 {
        let mut w = lock(&self.pairing);
        let left = match w.as_mut() {
            Some(p) => {
                p.attempts_left = p.attempts_left.saturating_sub(1);
                p.attempts_left
            }
            None => 0,
        };
        if left == 0 {
            *w = None;
        }
        left
    }

    /// One code, one computer.
    pub fn pairing_done(&self) {
        *lock(&self.pairing) = None;
    }

    // ── transport state ──

    pub fn set_runtime(&self, r: Runtime) {
        *lock(&self.runtime) = r;
    }

    pub fn runtime(&self) -> Runtime {
        lock(&self.runtime).clone()
    }

    pub fn connection_opened(&self, device_id: &str) {
        *lock(&self.connected).entry(device_id.to_string()).or_insert(0) += 1;
        self.publish("lan", "connect");
    }

    pub fn connection_closed(&self, device_id: &str) {
        let mut c = lock(&self.connected);
        if let Some(n) = c.get_mut(device_id) {
            *n -= 1;
            if *n == 0 {
                c.remove(device_id);
            }
        }
        drop(c);
        self.publish("lan", "disconnect");
    }

    pub fn is_connected(&self, device_id: &str) -> bool {
        lock(&self.connected).contains_key(device_id)
    }

    pub fn connected_count(&self) -> u32 {
        lock(&self.connected).len() as u32
    }

    // ── presence (7.3) ──

    fn viewers(
        map: &mut HashMap<(String, String), Vec<Viewer>>,
        key: &(String, String),
    ) -> Vec<PresenceViewer> {
        let Some(list) = map.get_mut(key) else { return Vec::new() };
        list.retain(|v| v.seen.elapsed() < PRESENCE_TTL);
        let out = list
            .iter()
            .map(|v| PresenceViewer {
                user_id: v.user_id.clone(),
                user_name: v.user_name.clone(),
                device: v.device.clone(),
                seconds: v.seen.elapsed().as_secs() as u32,
            })
            .collect();
        if list.is_empty() {
            map.remove(key);
        }
        out
    }

    /// Records (or refreshes) "this user, on this computer, has the record open".
    pub fn presence_enter(&self, p: &PresenceParams, user_id: &str, user_name: &str, device: &str) {
        let key = (p.entity.clone(), p.id.clone());
        let mut map = lock(&self.presence);
        let list = map.entry(key).or_default();
        let known = match list.iter_mut().find(|v| v.user_id == user_id && v.device == device) {
            Some(v) => {
                v.seen = Instant::now();
                true
            }
            None => false,
        };
        if !known {
            list.push(Viewer {
                user_id: user_id.into(),
                user_name: user_name.into(),
                device: device.into(),
                seen: Instant::now(),
            });
        }
        drop(map);
        if !known {
            self.publish("presence", "enter");
        }
    }

    pub fn presence_leave(&self, p: &PresenceParams, user_id: &str, device: &str) {
        let key = (p.entity.clone(), p.id.clone());
        let mut map = lock(&self.presence);
        let mut changed = false;
        if let Some(list) = map.get_mut(&key) {
            let before = list.len();
            list.retain(|v| !(v.user_id == user_id && v.device == device));
            changed = list.len() != before;
            if list.is_empty() {
                map.remove(&key);
            }
        }
        drop(map);
        if changed {
            self.publish("presence", "leave");
        }
    }

    /// Everybody who has the record open, except `except` (the asking user on the asking computer).
    pub fn presence_who(&self, p: &PresenceParams, except: Option<(&str, &str)>) -> Vec<PresenceViewer> {
        let key = (p.entity.clone(), p.id.clone());
        let mut all = Self::viewers(&mut lock(&self.presence), &key);
        if let Some((user_id, device)) = except {
            all.retain(|v| !(v.user_id == user_id && v.device == device));
        }
        all
    }

    /// Forgets everything a user had open (sign-out).
    pub fn presence_drop_user(&self, user_id: &str) {
        let mut map = lock(&self.presence);
        for list in map.values_mut() {
            list.retain(|v| v.user_id != user_id);
        }
        map.retain(|_, l| !l.is_empty());
    }
}

/// Whether a successful call changed something other screens may show. Reads, sessions,
/// drafts and the LAN pages themselves never fan out.
pub fn is_change(method: &str) -> bool {
    let Some((topic, action)) = method.split_once('.') else { return false };
    const QUIET_TOPICS: &[&str] = &[
        "app",
        "auth",
        "session",
        "drafts",
        "presence",
        "lan",
        "system",
        "update",
        "geo",
        "permissions",
        "reference",
        "audit",
    ];
    const READS: &[&str] = &[
        "list",
        "get",
        "counts",
        "info",
        "check",
        "check_duplicate",
        "billable",
        "account",
        "debtors",
        "statement",
        "installments_due",
        "summary",
        "settings_get",
        "report",
        "closings",
        "day",
        "tooth_history",
        "thumbnails",
        "file",
        "suggest_next",
        "open",
        "import_inspect",
        "export",
        "state",
        "touch",
        "rules_list",
    ];
    !QUIET_TOPICS.contains(&topic) && !READS.contains(&action)
}

fn read_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

fn write_setting(conn: &Connection, key: &str, value: &str, by: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT INTO setting(key, value, updated_at, updated_by) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at,
                                        updated_by = excluded.updated_by",
        params![key, value, now_iso(), by],
    )?;
    Ok(())
}

fn client_limit(conn: &Connection) -> Result<u32> {
    Ok(read_setting(conn, "lan_client_limit")?.and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_CLIENT_LIMIT))
}

fn fingerprint_text(cert_der: &[u8]) -> String {
    let h = Sha256::digest(cert_der);
    h[..8].chunks(2).map(hex::encode).collect::<Vec<_>>().join("-").to_uppercase()
}

fn device_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LanDevice> {
    let revoked: Option<String> = r.get(6)?;
    Ok(LanDevice {
        id: r.get(0)?,
        name: r.get(1)?,
        paired_at: r.get(2)?,
        last_seen_at: r.get(3)?,
        last_user: r.get(4)?,
        last_address: r.get(5)?,
        connected: false,
        revoked: revoked.is_some(),
    })
}

impl Core {
    pub fn lan(&self) -> &LanHub {
        &self.lan
    }

    /// Whether this computer is the clinic server.
    pub fn lan_enabled(&self) -> bool {
        self.with_db(|o| Ok(read_setting(&o.conn, "lan_server_enabled")?.as_deref() == Some("1")))
            .unwrap_or(false)
    }

    /// The server's TLS identity, if one was created: `(certificate DER, private key DER)`.
    pub fn lan_identity(&self) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        self.with_db(|o| {
            Ok(o.conn
                .query_row("SELECT cert_der, key_der FROM lan_identity WHERE id = 1", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()?)
        })
    }

    pub fn lan_store_identity(&self, cert_der: &[u8], key_der: &[u8]) -> Result<()> {
        self.with_db(|o| {
            o.conn.execute(
                "INSERT OR REPLACE INTO lan_identity(id, cert_der, key_der, created_at) VALUES (1, ?1, ?2, ?3)",
                params![cert_der, key_der, now_iso()],
            )?;
            Ok(())
        })
    }

    /// The name clients show for this server.
    pub fn lan_server_name(&self) -> String {
        self.clinic_name().unwrap_or_else(|| audit::computer_name().to_string())
    }

    /// Adds a computer that proved the pairing code. Fails once the plan's limit is reached.
    pub fn lan_register_device(&self, name: &str, token_hash: &str) -> Result<String> {
        let id = new_id();
        let name: String = name.trim().chars().take(60).collect();
        let name = if name.is_empty() { "Computer".to_string() } else { name };
        self.with_tx(|c| {
            let active: u32 =
                c.query_row("SELECT COUNT(*) FROM lan_device WHERE revoked_at IS NULL", [], |r| r.get(0))?;
            if active >= client_limit(c)? {
                return Err(CoreError::api(ErrorCode::Forbidden, "the plan's limit of computers is reached"));
            }
            c.execute(
                "INSERT INTO lan_device(id, name, token_hash, paired_at) VALUES (?1, ?2, ?3, ?4)",
                params![id, name, token_hash, now_iso()],
            )?;
            audit::record(
                c,
                &Actor::system(),
                "lan.pair",
                Some("lan_device"),
                Some(&id),
                None,
                Some(&serde_json::json!({ "name": name })),
            )?;
            Ok(())
        })?;
        self.lan.publish("lan", "pair");
        Ok(id)
    }

    /// A paired computer connects: returns its name when the token is right and it is not revoked.
    pub fn lan_authenticate(
        &self,
        device_id: &str,
        token_hash: &str,
        address: &str,
    ) -> Result<Option<String>> {
        self.with_db(|o| {
            let row: Option<(String, String, Option<String>)> = o
                .conn
                .query_row(
                    "SELECT name, token_hash, revoked_at FROM lan_device WHERE id = ?1",
                    [device_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((name, stored, revoked)) = row else { return Ok(None) };
            if revoked.is_some() || stored != token_hash {
                return Ok(None);
            }
            o.conn.execute(
                "UPDATE lan_device SET last_seen_at = ?2, last_address = ?3 WHERE id = ?1",
                params![device_id, now_iso(), address],
            )?;
            Ok(Some(name))
        })
    }

    /// Still paired and not revoked (checked before every request of a connection).
    pub fn lan_device_active(&self, device_id: &str) -> bool {
        self.with_db(|o| {
            Ok(o.conn.query_row(
                "SELECT COUNT(*) FROM lan_device WHERE id = ?1 AND revoked_at IS NULL",
                [device_id],
                |r| r.get::<_, u32>(0),
            )? > 0)
        })
        .unwrap_or(false)
    }

    /// Remembers who signed in on a computer (shown in the list of computers).
    pub fn lan_note_user(&self, device_id: &str, user: &str) {
        let _ = self.with_db(|o| {
            o.conn.execute(
                "UPDATE lan_device SET last_user = ?2, last_seen_at = ?3 WHERE id = ?1",
                params![device_id, user, now_iso()],
            )?;
            Ok(())
        });
    }

    fn lan_status(&self) -> Result<LanStatus> {
        let (enabled, limit, paired, fingerprint) = self.with_db(|o| {
            let enabled = read_setting(&o.conn, "lan_server_enabled")?.as_deref() == Some("1");
            let paired: u32 =
                o.conn.query_row("SELECT COUNT(*) FROM lan_device WHERE revoked_at IS NULL", [], |r| {
                    r.get(0)
                })?;
            let cert: Option<Vec<u8>> = o
                .conn
                .query_row("SELECT cert_der FROM lan_identity WHERE id = 1", [], |r| r.get(0))
                .optional()?;
            Ok((enabled, client_limit(&o.conn)?, paired, cert.map(|c| fingerprint_text(&c))))
        })?;
        let rt = self.lan.runtime();
        Ok(LanStatus {
            enabled,
            running: enabled && rt.running,
            port: if enabled && rt.running { rt.port } else { None },
            addresses: if enabled && rt.running { rt.addresses } else { Vec::new() },
            computer_name: audit::computer_name().to_string(),
            fingerprint,
            client_limit: limit,
            paired_count: paired,
            connected_count: self.lan.connected_count(),
            pairing: if enabled { self.lan.pairing_info() } else { None },
        })
    }

    fn lan_set_enabled(&self, s: &Session, on: bool) -> Result<LanStatus> {
        self.with_tx(|c| {
            write_setting(c, "lan_server_enabled", if on { "1" } else { "0" }, Some(&s.user.id))?;
            audit::record(
                c,
                &Actor::user(&s.user.id, &s.user.username),
                if on { "lan.enable" } else { "lan.disable" },
                Some("setting"),
                Some("lan_server_enabled"),
                None,
                None,
            )
        })?;
        if !on {
            self.lan.cancel_pairing();
        }
        self.lan.publish("lan", if on { "enable" } else { "disable" });
        self.lan_status()
    }

    /// `lan.*` and `presence.*`; `origin` is the computer the request came from.
    pub(crate) fn call_lan(&self, method: &str, p: Value, s: &Session, origin: &str) -> Result<Value> {
        match method {
            m::PRESENCE_ENTER => {
                let p: PresenceParams = parse(p)?;
                self.lan.presence_enter(&p, &s.user.id, &s.user.display_name, origin);
                to_value(self.lan.presence_who(&p, Some((&s.user.id, origin))))
            }
            m::PRESENCE_LEAVE => {
                let p: PresenceParams = parse(p)?;
                self.lan.presence_leave(&p, &s.user.id, origin);
                to_value(artaveo_shared::Empty {})
            }
            m::PRESENCE_WHO => {
                let p: PresenceParams = parse(p)?;
                to_value(self.lan.presence_who(&p, Some((&s.user.id, origin))))
            }
            m::LAN_STATUS => {
                s.require(perm::SETTINGS_MANAGE)?;
                to_value(self.lan_status()?)
            }
            m::LAN_ENABLE => {
                s.require(perm::SETTINGS_MANAGE)?;
                to_value(self.lan_set_enabled(s, true)?)
            }
            m::LAN_DISABLE => {
                s.require(perm::SETTINGS_MANAGE)?;
                to_value(self.lan_set_enabled(s, false)?)
            }
            m::LAN_PAIRING_OPEN => {
                s.require(perm::SETTINGS_MANAGE)?;
                if !self.lan_enabled() {
                    return Err(CoreError::invalid(
                        "enabled",
                        ValidationRule::Required,
                        "turn on server mode before adding a computer",
                    ));
                }
                let info = self.lan.open_pairing();
                self.with_db(|o| {
                    audit::record(
                        &o.conn,
                        &Actor::user(&s.user.id, &s.user.username),
                        "lan.pairing_open",
                        None,
                        None,
                        None,
                        None,
                    )
                })?;
                to_value(info)
            }
            m::LAN_PAIRING_CANCEL => {
                s.require(perm::SETTINGS_MANAGE)?;
                self.lan.cancel_pairing();
                to_value(artaveo_shared::Empty {})
            }
            m::LAN_DEVICES => {
                s.require(perm::SETTINGS_MANAGE)?;
                let mut list = self.with_db(|o| {
                    let mut st = o.conn.prepare(
                        "SELECT id, name, paired_at, last_seen_at, last_user, last_address, revoked_at
                         FROM lan_device ORDER BY revoked_at IS NOT NULL, paired_at",
                    )?;
                    let rows = st.query_map([], device_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
                    Ok(rows)
                })?;
                for d in &mut list {
                    d.connected = !d.revoked && self.lan.is_connected(&d.id);
                }
                to_value(list)
            }
            m::LAN_REVOKE => {
                s.require(perm::SETTINGS_MANAGE)?;
                let artaveo_shared::UserIdParams { id } = parse(p)?;
                self.with_tx(|c| {
                    let n = c.execute(
                        "UPDATE lan_device SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
                        params![id, now_iso()],
                    )?;
                    if n == 0 {
                        return Err(CoreError::api(ErrorCode::NotFound, "no such computer"));
                    }
                    audit::record(
                        c,
                        &Actor::user(&s.user.id, &s.user.username),
                        "lan.revoke",
                        Some("lan_device"),
                        Some(&id),
                        None,
                        None,
                    )
                })?;
                // Live connections of that computer re-check and close.
                self.lan.publish("lan", "revoke");
                to_value(artaveo_shared::Empty {})
            }
            _ => unreachable!("lan method list and match are in sync"),
        }
    }
}

fn parse<P: serde::de::DeserializeOwned>(v: Value) -> Result<P> {
    let v = if v.is_null() { serde_json::json!({}) } else { v };
    serde_json::from_value(v).map_err(|e| CoreError::validation(format!("invalid params: {e}")))
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value> {
    Ok(serde_json::to_value(v)?)
}

pub(crate) fn is_lan_method(method: &str) -> bool {
    method.starts_with("lan.") || method.starts_with("presence.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_session_calls_do_not_fan_out() {
        assert!(is_change("patients.update"));
        assert!(is_change("appointments.set_status"));
        assert!(is_change("payments.add"));
        assert!(!is_change("patients.list"));
        assert!(!is_change("session.touch"));
        assert!(!is_change("auth.login"));
        assert!(!is_change("presence.enter"));
        assert!(!is_change("lan.revoke"));
        assert!(!is_change("billing.summary"));
        assert!(!is_change("drafts.save"));
    }

    #[test]
    fn pairing_window_counts_attempts_and_closes() {
        let hub = LanHub::new();
        assert!(hub.pairing_info().is_none());
        let info = hub.open_pairing();
        assert_eq!(info.code.len(), 6);
        assert_eq!(info.attempts_left, MAX_PAIRING_ATTEMPTS);
        assert_eq!(hub.pairing_code().as_deref(), Some(info.code.as_str()));
        for left in (0..MAX_PAIRING_ATTEMPTS).rev() {
            assert_eq!(hub.pairing_failed(), left);
        }
        assert!(hub.pairing_code().is_none(), "closed after the last wrong attempt");
        hub.open_pairing();
        hub.pairing_done();
        assert!(hub.pairing_info().is_none(), "one code, one computer");
    }

    #[test]
    fn presence_lists_other_screens_only() {
        let hub = LanHub::new();
        let rec = PresenceParams { entity: "patient".into(), id: "p1".into() };
        hub.presence_enter(&rec, "u1", "Reception", "PC-A");
        hub.presence_enter(&rec, "u2", "Dr. Zarghona", "PC-B");
        let seen_by_reception = hub.presence_who(&rec, Some(("u1", "PC-A")));
        assert_eq!(seen_by_reception.len(), 1);
        assert_eq!(seen_by_reception[0].user_name, "Dr. Zarghona");
        hub.presence_leave(&rec, "u2", "PC-B");
        assert!(hub.presence_who(&rec, Some(("u1", "PC-A"))).is_empty());
        hub.presence_drop_user("u1");
        assert!(hub.presence_who(&rec, None).is_empty());
    }

    fn clinic_core() -> (tempfile::TempDir, Core) {
        let dir = tempfile::tempdir().unwrap();
        let core =
            Core::open(crate::Config::new(crate::Environment::Test, dir.path().to_path_buf())).unwrap();
        let r = core.handle(artaveo_shared::RpcRequest {
            method: m::APP_SETUP.into(),
            params: serde_json::json!({
                "clinic_name": "Test", "owner_username": "owner", "owner_display_name": "Owner",
                "owner_password": "owner-pass-1", "language": "en"
            }),
            token: None,
        });
        assert!(matches!(r, artaveo_shared::RpcResponse::Ok { .. }));
        (dir, core)
    }

    #[test]
    fn the_plans_client_limit_is_enforced_and_revoking_frees_a_place() {
        let (_dir, core) = clinic_core();
        core.with_db(|o| write_setting(&o.conn, "lan_client_limit", "2", None)).unwrap();
        let a = core.lan_register_device("A", "hash-a").unwrap();
        core.lan_register_device("B", "hash-b").unwrap();
        let full = core.lan_register_device("C", "hash-c").unwrap_err();
        assert_eq!(full.code(), ErrorCode::Forbidden);

        assert_eq!(core.lan_authenticate(&a, "hash-a", "10.0.0.2").unwrap().as_deref(), Some("A"));
        assert_eq!(core.lan_authenticate(&a, "wrong", "10.0.0.2").unwrap(), None);
        assert!(core.lan_device_active(&a));

        core.with_db(|o| {
            o.conn.execute("UPDATE lan_device SET revoked_at = ?2 WHERE id = ?1", params![a, now_iso()])?;
            Ok(())
        })
        .unwrap();
        assert!(!core.lan_device_active(&a));
        assert_eq!(core.lan_authenticate(&a, "hash-a", "10.0.0.2").unwrap(), None, "revoked");
        core.lan_register_device("C", "hash-c").expect("a place is free again");
    }
}
