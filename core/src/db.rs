//! Encrypted local database (ADR-03): SQLCipher + WAL + FK + STRICT,
//! transactional checksummed migrations with a backup before every migration.
//! Promoted from the Phase 0 `sqlcipher-core` spike.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::error::{CoreError, Result};
use crate::keys::{DataKey, KeyProtector, RecoveryKey};

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

impl Migration {
    pub fn checksum(&self) -> String {
        hex::encode(Sha256::digest(self.sql.as_bytes()))
    }
}

pub static MIGRATIONS: &[Migration] = &[
    Migration { version: 1, name: "foundation", sql: include_str!("../migrations/0001_foundation.sql") },
    Migration { version: 2, name: "identity", sql: include_str!("../migrations/0002_identity.sql") },
    Migration { version: 3, name: "reference", sql: include_str!("../migrations/0003_reference.sql") },
    Migration { version: 4, name: "backup", sql: include_str!("../migrations/0004_backup.sql") },
    Migration { version: 5, name: "patient", sql: include_str!("../migrations/0005_patient.sql") },
    Migration { version: 6, name: "scheduling", sql: include_str!("../migrations/0006_scheduling.sql") },
    Migration {
        version: 7,
        name: "patient_birth_year",
        sql: include_str!("../migrations/0007_patient_birth_year.sql"),
    },
    Migration { version: 8, name: "form_draft", sql: include_str!("../migrations/0008_form_draft.sql") },
    Migration {
        version: 9,
        name: "role_customized",
        sql: include_str!("../migrations/0009_role_customized.sql"),
    },
    Migration {
        version: 10,
        name: "medical_checklist",
        sql: include_str!("../migrations/0010_medical_checklist.sql"),
    },
];

/// Files next to the database.
#[derive(Debug, Clone)]
pub struct KeyFiles {
    /// Data key wrapped by DPAPI (machine-bound).
    pub local: PathBuf,
    /// Data key wrapped by the Recovery Key (portable; copied into every backup).
    pub recovery: PathBuf,
}

impl KeyFiles {
    pub fn beside(db: &Path) -> Self {
        Self { local: db.with_extension("key"), recovery: db.with_extension("rkey") }
    }
}

/// Opens a connection with the clinic key and the mandatory PRAGMAs.
pub fn connect(path: &Path, key: &DataKey) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(&format!("PRAGMA key = {};", key.sqlcipher_pragma_value()))?;
    // Reading the schema is what validates the key.
    if conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).is_err() {
        return Err(CoreError::WrongKey);
    }
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "FULL")?; // NFR: zero loss of committed transactions
    conn.pragma_update(None, "busy_timeout", 5000)?;
    Ok(conn)
}

/// First run: new random data key, stored DPAPI-protected and Recovery-wrapped.
pub fn create(
    path: &Path,
    protector: &dyn KeyProtector,
    recovery: &RecoveryKey,
) -> Result<(Connection, DataKey)> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let key = DataKey::generate();
    let files = KeyFiles::beside(path);
    fs::write(&files.local, protector.protect(&key)?)?;
    fs::write(&files.recovery, recovery.wrap(&key))?;
    Ok((connect(path, &key)?, key))
}

pub fn load_key(path: &Path, protector: &dyn KeyProtector) -> Result<DataKey> {
    Ok(protector.unprotect(&fs::read(KeyFiles::beside(path).local)?)?)
}

/// Fast start-up validation only (ADR-03): key works and schema is readable.
/// The full page-level check runs in the background ([`full_integrity_check`]).
pub fn quick_validate(conn: &Connection) -> Result<()> {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))?;
    Ok(())
}

/// `cipher_integrity_check` (HMAC of every page) + `quick_check` + FK check.
/// About 13 ms/MB — run off the UI path.
pub fn full_integrity_check(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA cipher_integrity_check")?;
    let errors: Vec<String> =
        stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<_>>()?;
    if !errors.is_empty() {
        return Err(CoreError::Integrity(errors.join("; ")));
    }
    let quick: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if quick != "ok" {
        return Err(CoreError::Integrity(quick));
    }
    let fk: i64 = conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
    if fk > 0 {
        return Err(CoreError::Integrity(format!("{fk} foreign key violations")));
    }
    Ok(())
}

fn ensure_migrations_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            checksum   TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        ) STRICT;",
    )?;
    Ok(())
}

pub fn schema_version(conn: &Connection) -> Result<i64> {
    ensure_migrations_table(conn)?;
    Ok(conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?)
}

/// Pending migrations after checking applied ones were not edited.
pub fn pending<'a>(conn: &Connection, set: &'a [Migration]) -> Result<Vec<&'a Migration>> {
    ensure_migrations_table(conn)?;
    let known_max = set.iter().map(|m| m.version).max().unwrap_or(0);
    let db_max = schema_version(conn)?;
    if db_max > known_max {
        return Err(CoreError::DatabaseNewerThanApp { db: db_max, app: known_max });
    }
    let mut out = Vec::new();
    for m in set {
        let applied: Option<String> = conn
            .query_row("SELECT checksum FROM schema_migrations WHERE version = ?1", [m.version], |r| r.get(0))
            .optional()?;
        match applied {
            Some(sum) if sum != m.checksum() => return Err(CoreError::MigrationDrift { version: m.version }),
            Some(_) => {}
            None => out.push(m),
        }
    }
    Ok(out)
}

/// Applies pending migrations, each in its own IMMEDIATE transaction.
/// If the database already has a schema, an encrypted backup is written to
/// `backup_dir` first and its path returned.
pub fn migrate(conn: &mut Connection, set: &[Migration], backup_dir: &Path) -> Result<Option<PathBuf>> {
    let todo = pending(conn, set)?;
    if todo.is_empty() {
        return Ok(None);
    }
    let from = schema_version(conn)?;
    let backup = if from > 0 {
        fs::create_dir_all(backup_dir)?;
        let p = backup_dir.join(format!(
            "pre-migration-v{}-to-v{}-{}.db",
            from,
            todo.last().unwrap().version,
            crate::clock::now().unix_timestamp()
        ));
        vacuum_into(conn, &p)?;
        Some(p)
    } else {
        None
    };
    for m in todo {
        tracing::info!(version = m.version, name = m.name, "applying migration");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(m.sql).map_err(|e| CoreError::MigrationFailed { version: m.version, source: e })?;
        tx.execute(
            "INSERT INTO schema_migrations(version, name, checksum) VALUES (?1, ?2, ?3)",
            params![m.version, m.name, m.checksum()],
        )?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
    }
    Ok(backup)
}

/// Consistent encrypted snapshot (same key) that does not block writers.
pub fn vacuum_into(conn: &Connection, dest: &Path) -> Result<()> {
    if dest.exists() {
        fs::remove_file(dest)?;
    }
    conn.execute("VACUUM INTO ?1", [dest.to_string_lossy()])?;
    Ok(())
}

/// Bytes of the database pages (the main file once checkpointed), without the WAL (OF-024).
/// The main database file's size on disk (OF-024). Reading the file avoids `PRAGMA page_size`,
/// which SQLCipher can answer as text (`cipher_page_size`) depending on the connection's state.
pub fn data_size_bytes(db_path: &Path) -> Result<i64> {
    Ok(std::fs::metadata(db_path)?.len() as i64)
}

pub fn cipher_version(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("PRAGMA cipher_version", [], |r| r.get(0))?)
}

pub fn sqlite_version(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?)
}

/// Optimistic locking (Data Conventions 1.4): an UPDATE that bumps
/// `version` must have matched exactly one row, otherwise someone else saved
/// first (or the row is gone).
pub fn expect_one_row(changed: usize, entity: &str) -> Result<()> {
    if changed == 1 {
        Ok(())
    } else {
        Err(CoreError::api(
            artaveo_shared::ErrorCode::Conflict,
            format!("{entity} was changed by another user or no longer exists"),
        ))
    }
}
