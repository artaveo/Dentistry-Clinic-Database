use std::fs;
use std::path::{Path, PathBuf};

use key_protect::{DataKey, KeyError, KeyProtector, RecoveryKey};
use rusqlite::Connection;

use crate::migrate::{self, Migration};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Key(#[from] KeyError),
    #[error("database key is wrong or file is not an Artaveo database")]
    WrongKey,
    #[error("integrity check failed: {0}")]
    Integrity(String),
    #[error("migration {version} was modified after being applied")]
    MigrationDrift { version: i64 },
    #[error("migration {version} failed: {source}")]
    MigrationFailed { version: i64, source: rusqlite::Error },
    #[error("database schema v{db} is newer than this app (v{app}); update the app")]
    DatabaseNewerThanApp { db: i64, app: i64 },
}

/// Files that live next to the database in the app data folder.
#[derive(Debug, Clone)]
pub struct KeyFiles {
    /// Data key wrapped by the OS protector (DPAPI on Windows). Machine-bound.
    pub local: PathBuf,
    /// Data key wrapped by the Recovery Key. Portable; copied into every backup.
    pub recovery: PathBuf,
}

impl KeyFiles {
    pub fn beside(db: &Path) -> Self {
        Self { local: db.with_extension("key"), recovery: db.with_extension("rkey") }
    }
}

pub struct Database {
    pub conn: Connection,
    pub path: PathBuf,
}

fn configure(conn: &Connection, key: &DataKey) -> Result<(), StoreError> {
    // Must be the first statement on the connection.
    conn.execute_batch(&format!("PRAGMA key = {};", key.sqlcipher_pragma_value()))?;
    // Touching the schema is what actually validates the key.
    if conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).is_err() {
        return Err(StoreError::WrongKey);
    }
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "FULL")?; // NFR: zero loss for committed tx
    conn.pragma_update(None, "busy_timeout", 5000)?;
    Ok(())
}

impl Database {
    /// First run: generate a data key, store it DPAPI-protected and
    /// recovery-wrapped, create the encrypted DB and migrate.
    pub fn create(
        path: &Path,
        protector: &dyn KeyProtector,
        recovery: &RecoveryKey,
    ) -> Result<Self, StoreError> {
        let key = DataKey::generate();
        let files = KeyFiles::beside(path);
        fs::write(&files.local, protector.protect(&key)?)?;
        fs::write(&files.recovery, recovery.wrap(&key))?;
        let mut db = Self::open_with_key(path, &key)?;
        db.migrate(migrate::MIGRATIONS)?;
        Ok(db)
    }

    /// Normal startup: unwrap key with the OS protector, open, check, migrate.
    pub fn open(path: &Path, protector: &dyn KeyProtector) -> Result<Self, StoreError> {
        let blob = fs::read(KeyFiles::beside(path).local)?;
        let key = protector.unprotect(&blob)?;
        let mut db = Self::open_with_key(path, &key)?;
        db.startup_checks()?;
        db.migrate(migrate::MIGRATIONS)?;
        Ok(db)
    }

    /// New-PC restore: the machine-bound key file is useless here, so unwrap
    /// with the Recovery Key and re-protect for this machine.
    pub fn restore_with_recovery_key(
        path: &Path,
        protector: &dyn KeyProtector,
        recovery: &RecoveryKey,
    ) -> Result<Self, StoreError> {
        let files = KeyFiles::beside(path);
        let key = recovery.unwrap(&fs::read(&files.recovery)?)?;
        let mut db = Self::open_with_key(path, &key)?;
        db.startup_checks()?;
        fs::write(&files.local, protector.protect(&key)?)?;
        db.migrate(migrate::MIGRATIONS)?;
        Ok(db)
    }

    pub fn open_with_key(path: &Path, key: &DataKey) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        configure(&conn, key)?;
        Ok(Self { conn, path: path.to_path_buf() })
    }

    pub fn startup_checks(&self) -> Result<(), StoreError> {
        // HMAC check of every page (SQLCipher) — detects tampering/bit-rot.
        let mut stmt = self.conn.prepare("PRAGMA cipher_integrity_check")?;
        let errors: Vec<String> = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?;
        if !errors.is_empty() {
            return Err(StoreError::Integrity(errors.join("; ")));
        }
        let quick: String = self.conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if quick != "ok" {
            return Err(StoreError::Integrity(quick));
        }
        let fk_violations: i64 =
            self.conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
        if fk_violations > 0 {
            return Err(StoreError::Integrity(format!("{fk_violations} FK violations")));
        }
        Ok(())
    }

    /// Applies pending migrations; takes an encrypted backup first if any are pending.
    pub fn migrate(&mut self, set: &[Migration]) -> Result<Option<PathBuf>, StoreError> {
        let todo = migrate::pending(&self.conn, set)?;
        if todo.is_empty() {
            return Ok(None);
        }
        let from = migrate::current_version(&self.conn)?;
        let backup = if from > 0 {
            let p = self.path.with_extension(format!("pre-v{}.bak", todo[0].version));
            self.backup_to(&p)?;
            Some(p)
        } else {
            None
        };
        migrate::apply(&mut self.conn, &todo)?;
        Ok(backup)
    }

    /// Consistent, still-encrypted snapshot (same key) via `VACUUM INTO`.
    /// Works while other connections are reading/writing in WAL mode.
    pub fn backup_to(&self, dest: &Path) -> Result<(), StoreError> {
        if dest.exists() {
            fs::remove_file(dest)?;
        }
        self.conn.execute("VACUUM INTO ?1", [dest.to_string_lossy()])?;
        Ok(())
    }

    pub fn sqlcipher_version(&self) -> Result<String, StoreError> {
        Ok(self.conn.query_row("PRAGMA cipher_version", [], |r| r.get(0))?)
    }

    pub fn new_id() -> String {
        uuid::Uuid::now_v7().to_string()
    }
}
