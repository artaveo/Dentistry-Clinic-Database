use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use artaveo_shared::{BackupInfo, ErrorCode, IntegrityInfo, RpcRequest, RpcResponse};
use rusqlite::{Connection, OptionalExtension};

use crate::audit::{self, Actor};
use crate::backup;
use crate::clock::{now, now_iso};
use crate::config::Config;
use crate::db;
use crate::error::{CoreError, Result};
use crate::keys::{default_protector, DataKey, KeyProtector};
use crate::lan::{self, LanHub};
use crate::license::{trusted_keys, HttpServer, LicenseManager};
use crate::session::SessionStore;
use crate::settings;
use crate::usb::{SystemUsb, UsbProvider};

pub(crate) struct Opened {
    pub conn: Connection,
    pub key: DataKey,
}

/// The application core. One instance per process; all transports share it.
pub struct Core {
    pub(crate) config: Config,
    pub(crate) protector: Box<dyn KeyProtector>,
    pub(crate) db: Mutex<Option<Opened>>,
    pub(crate) sessions: Mutex<SessionStore>,
    pub(crate) timeout_minutes: AtomicU32,
    pub(crate) integrity: Arc<Mutex<IntegrityInfo>>,
    pub(crate) lan: LanHub,
    /// Where the connected USB drives come from (a fake one in tests).
    pub(crate) usb: Mutex<Arc<dyn UsbProvider>>,
    pub(crate) license: LicenseManager,
}

impl Core {
    pub fn open(config: Config) -> Result<Self> {
        Self::with_protector(config, default_protector())
    }

    /// Opens an existing clinic database (or starts in "needs setup" state):
    /// key unwrap → quick validation → migrations (backup first) → seed and
    /// role sync → background full integrity check.
    pub fn with_protector(config: Config, protector: Box<dyn KeyProtector>) -> Result<Self> {
        Self::build(config, protector, true)
    }

    /// A core that does not load the clinic database even though one may exist. Safe Startup Mode
    /// (roadmap 11.3) uses it to restore a backup with the Recovery Key when the database or its key
    /// on this computer is unusable; the damaged files are moved aside, never deleted.
    pub fn open_for_recovery(config: Config) -> Result<Self> {
        Self::build(config, default_protector(), false)
    }

    fn build(config: Config, protector: Box<dyn KeyProtector>, load_database: bool) -> Result<Self> {
        fs::create_dir_all(&config.data_dir)?;
        // A production build trusts only the production key and needs a license; development and tests do not.
        let production = config.environment == crate::Environment::Production;
        let license = LicenseManager::new(
            &config.data_dir,
            trusted_keys(!production),
            Box::new(HttpServer::from_build(production)),
            crate::fingerprint::current(),
            !production,
        )?;
        let core = Core {
            config,
            protector,
            db: Mutex::new(None),
            sessions: Mutex::new(SessionStore::default()),
            timeout_minutes: AtomicU32::new(settings::DEFAULTS.session_timeout_minutes),
            integrity: Arc::new(Mutex::new(IntegrityInfo {
                status: "pending".into(),
                checked_at: None,
                detail: None,
            })),
            lan: LanHub::new(),
            usb: Mutex::new(Arc::new(SystemUsb)),
            license,
        };
        let path = core.config.db_path();
        if load_database && path.exists() {
            let key = db::load_key(&path, core.protector.as_ref())?;
            let conn = db::connect(&path, &key)?;
            db::quick_validate(&conn)?;
            core.install(conn, key)?;
            core.start_integrity_check();
        }
        Ok(core)
    }

    /// Runs migrations and system-data sync, then makes the DB available.
    pub(crate) fn install(&self, mut conn: Connection, key: DataKey) -> Result<()> {
        let before = db::schema_version(&conn)?;
        let pre_backup = db::migrate(&mut conn, db::MIGRATIONS, &self.config.backup_dir())?;
        let after = db::schema_version(&conn)?;
        crate::seeds::sync(&mut conn)?;
        crate::auth::sync_roles(&mut conn)?;
        if before > 0 && after != before {
            audit::record(
                &conn,
                &Actor::system(),
                "db.migrate",
                None,
                None,
                Some(&serde_json::json!({ "schema_version": before })),
                Some(
                    &serde_json::json!({ "schema_version": after, "backup": pre_backup.map(|p| p.display().to_string()) }),
                ),
            )?;
        }
        self.timeout_minutes.store(settings::get(&conn)?.session_timeout_minutes, Ordering::Relaxed);
        *self.lock_db() = Some(Opened { conn, key });
        self.apply_license_limits();
        Ok(())
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn is_set_up(&self) -> bool {
        self.lock_db().is_some()
    }

    pub(crate) fn lock_db(&self) -> MutexGuard<'_, Option<Opened>> {
        self.db.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub(crate) fn lock_sessions(&self) -> MutexGuard<'_, SessionStore> {
        self.sessions.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Runs `f` with the open database, or fails with `NotSetUp`.
    pub(crate) fn with_db<T>(&self, f: impl FnOnce(&mut Opened) -> Result<T>) -> Result<T> {
        let mut guard = self.lock_db();
        let opened =
            guard.as_mut().ok_or_else(|| CoreError::api(ErrorCode::NotSetUp, "clinic is not set up"))?;
        f(opened)
    }

    /// Like [`with_db`](Self::with_db), but everything `f` writes is one atomic
    /// transaction: any error rolls it all back (appointment + recall + audit
    /// either all happen or none does).
    pub(crate) fn with_tx<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        self.with_db(|o| {
            let tx = o.conn.transaction()?;
            let out = f(&tx)?;
            tx.commit()?;
            Ok(out)
        })
    }

    /// [`with_tx`](Self::with_tx) for work that also needs the data key (attachment files).
    pub(crate) fn with_tx_keyed<T>(&self, f: impl FnOnce(&Connection, &DataKey) -> Result<T>) -> Result<T> {
        self.with_db(|o| {
            let tx = o.conn.transaction()?;
            let out = f(&tx, &o.key)?;
            tx.commit()?;
            Ok(out)
        })
    }

    pub(crate) fn session_timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_minutes.load(Ordering::Relaxed) as u64 * 60)
    }

    /// Single entry point for every transport.
    pub fn handle(&self, req: RpcRequest) -> RpcResponse {
        self.handle_from(req, crate::audit::computer_name())
    }

    /// Like [`handle`](Self::handle), for a request that arrived from the computer called `origin`
    /// (the LAN server passes the name of the paired computer). A change is announced to every
    /// connected screen so they can refresh (roadmap 7.3).
    pub fn handle_from(&self, req: RpcRequest, origin: &str) -> RpcResponse {
        let method = req.method.clone();
        match self.call(req, origin) {
            Ok(result) => {
                if lan::is_change(&method) {
                    if let Some((topic, _)) = method.split_once('.') {
                        self.lan.publish(topic, &method);
                    }
                }
                RpcResponse::Ok { result }
            }
            Err(e) => {
                let mut error = e.to_rpc();
                if e.code() == ErrorCode::Internal {
                    // A short reference the clinic can read out on the phone: support searches the log
                    // for it and lands on this exact failure (method, computer, error text).
                    let reference = error_reference();
                    tracing::error!(%method, %reference, %origin, error = %e, "request failed");
                    error.detail = format!("{} [ref {reference}]", error.detail);
                } else {
                    tracing::debug!(%method, error = %e, "request rejected");
                }
                RpcResponse::Error { error }
            }
        }
    }

    /// Full integrity check on a separate connection (ADR-03: not on the start-up path).
    pub fn start_integrity_check(&self) {
        let Some(key) = self.lock_db().as_ref().map(|o| o.key.clone()) else { return };
        let path = self.config.db_path();
        let state = self.integrity.clone();
        state.lock().unwrap().status = "running".into();
        std::thread::spawn(move || {
            let result = db::connect(&path, &key).and_then(|c| db::full_integrity_check(&c));
            let mut s = state.lock().unwrap();
            s.checked_at = Some(now_iso());
            match result {
                Ok(()) => {
                    s.status = "ok".into();
                    s.detail = None;
                }
                Err(e) => {
                    tracing::error!(error = %e, "database integrity check failed");
                    s.status = "failed".into();
                    s.detail = Some(e.to_string());
                }
            }
        });
    }

    pub fn integrity(&self) -> IntegrityInfo {
        self.integrity.lock().unwrap().clone()
    }

    pub(crate) fn usb(&self) -> Arc<dyn UsbProvider> {
        self.usb.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    #[cfg(test)]
    pub(crate) fn set_usb(&self, provider: Arc<dyn UsbProvider>) {
        *self.usb.lock().unwrap() = provider;
    }

    /// Scheduled work; the shell calls this every few minutes.
    /// Returns the backup made, if one was due. A backup that is not due still brings the newest
    /// backup to a USB drive or cloud folder that was missing when it was made (roadmap 10.4).
    pub fn run_due_tasks(&self) -> Result<Option<BackupInfo>> {
        let usb = self.usb();
        self.with_db(|o| {
            let s = backup::get_settings(&o.conn)?;
            if !backup::is_due(backup::latest(&o.conn)?.as_ref(), s.daily_hour, s.daily_minute, now()) {
                backup::transfer_pending(&o.conn, &o.key, &self.config, usb.as_ref())?;
                return Ok(None);
            }
            let info = backup::create(&o.conn, &o.key, &self.config, backup::Kind::Daily, &Actor::system())?;
            backup::distribute(&o.conn, &o.key, &self.config, usb.as_ref(), &info)?;
            backup::apply_retention(&o.conn, &self.config)?;
            Ok(Some(info))
        })
        .or_else(|e| if e.code() == ErrorCode::NotSetUp { Ok(None) } else { Err(e) })
    }

    /// The periodic license check (roadmap 10.8, about every 7 days while online); a failed attempt
    /// is only logged: the clinic keeps working through the grace period.
    pub fn run_license_check(&self) {
        if !self.is_set_up() || !self.license.should_check_now() {
            return;
        }
        match self.license.verify(&self.license_client_info()) {
            Ok(st) => {
                tracing::info!(state = ?st.state, "license verified");
                self.apply_license_limits();
            }
            Err(e) => tracing::debug!(error = %e, "license check skipped"),
        }
    }

    /// What this computer tells the license server about itself.
    pub(crate) fn license_client_info(&self) -> crate::license::ClientInfo {
        let (last, ok) = self
            .lock_db()
            .as_ref()
            .and_then(|o| backup::latest(&o.conn).ok().flatten())
            .map(|b| (Some(b.created_at), Some(b.verified)))
            .unwrap_or((None, None));
        crate::license::ClientInfo {
            clinic_name: self.clinic_name().unwrap_or_default(),
            computer_name: audit::computer_name().to_string(),
            app_version: crate::APP_VERSION.to_string(),
            arch: crate::sysinfo::build_arch().to_string(),
            last_backup_at: last,
            last_backup_ok: ok,
        }
    }

    /// The plan decides how many computers may join (roadmap 10.8): the server PC is one of them.
    pub(crate) fn apply_license_limits(&self) {
        let status = self.license.status();
        if status.read_only || status.state == artaveo_shared::LicenseState::Unactivated {
            return;
        }
        let clients = status.computers.saturating_sub(1).to_string();
        let _ = self.with_db(|o| {
            o.conn.execute(
                "INSERT INTO setting(key, value, updated_at, updated_by) VALUES ('lan_client_limit', ?1, ?2, NULL)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                rusqlite::params![clients, now_iso()],
            )?;
            Ok(())
        });
    }

    pub(crate) fn clinic_name(&self) -> Option<String> {
        self.lock_db().as_ref().and_then(|o| {
            o.conn
                .query_row("SELECT value FROM db_meta WHERE key = 'clinic_name'", [], |r| r.get(0))
                .optional()
                .ok()
                .flatten()
        })
    }
}

/// Six characters (no look-alikes) that tie a failure shown on screen to its line in the log.
fn error_reference() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..6).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect()
}
