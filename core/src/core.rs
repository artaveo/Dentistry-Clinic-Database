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
use crate::session::SessionStore;
use crate::settings;

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
}

impl Core {
    pub fn open(config: Config) -> Result<Self> {
        Self::with_protector(config, default_protector())
    }

    /// Opens an existing clinic database (or starts in "needs setup" state):
    /// key unwrap → quick validation → migrations (backup first) → seed and
    /// role sync → background full integrity check.
    pub fn with_protector(config: Config, protector: Box<dyn KeyProtector>) -> Result<Self> {
        fs::create_dir_all(&config.data_dir)?;
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
        };
        let path = core.config.db_path();
        if path.exists() {
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

    pub(crate) fn session_timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_minutes.load(Ordering::Relaxed) as u64 * 60)
    }

    /// Single entry point for every transport.
    pub fn handle(&self, req: RpcRequest) -> RpcResponse {
        let method = req.method.clone();
        match self.call(req) {
            Ok(result) => RpcResponse::Ok { result },
            Err(e) => {
                if e.code() == ErrorCode::Internal {
                    tracing::error!(%method, error = %e, "request failed");
                } else {
                    tracing::debug!(%method, error = %e, "request rejected");
                }
                RpcResponse::Error { error: e.to_rpc() }
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

    /// Scheduled work; the shell calls this every few minutes.
    /// Returns the backup made, if one was due.
    pub fn run_due_tasks(&self) -> Result<Option<BackupInfo>> {
        self.with_db(|o| {
            let s = settings::get(&o.conn)?;
            if !backup::is_due(backup::latest(&o.conn)?.as_ref(), &s, now()) {
                return Ok(None);
            }
            let info = backup::create(&o.conn, &o.key, &self.config, backup::Kind::Daily, &Actor::system())?;
            backup::apply_retention(&o.conn, &self.config.backup_dir(), s.backup_keep_daily)?;
            Ok(Some(info))
        })
        .or_else(|e| if e.code() == ErrorCode::NotSetUp { Ok(None) } else { Err(e) })
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
