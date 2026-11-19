//! Encrypted local database (ADR-03): SQLCipher + WAL + FK + STRICT,
//! transactional checksummed migrations with a backup before every migration.
//! Promoted from the Phase 0 `sqlcipher-core` spike.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
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
    Migration { version: 11, name: "documents", sql: include_str!("../migrations/0011_documents.sql") },
    Migration { version: 12, name: "catalog", sql: include_str!("../migrations/0012_catalog.sql") },
    Migration {
        version: 13,
        name: "clinical_core",
        sql: include_str!("../migrations/0013_clinical_core.sql"),
    },
    Migration { version: 14, name: "billing", sql: include_str!("../migrations/0014_billing.sql") },
    Migration { version: 15, name: "lan", sql: include_str!("../migrations/0015_lan.sql") },
    Migration { version: 16, name: "inventory", sql: include_str!("../migrations/0016_inventory.sql") },
    Migration { version: 17, name: "reports", sql: include_str!("../migrations/0017_reports.sql") },
    Migration {
        version: 18,
        name: "backup_manager",
        sql: include_str!("../migrations/0018_backup_manager.sql"),
    },
    Migration { version: 19, name: "avatars", sql: include_str!("../migrations/0019_avatars.sql") },
    Migration { version: 20, name: "db_size", sql: include_str!("../migrations/0020_db_size.sql") },
    Migration { version: 21, name: "finance", sql: include_str!("../migrations/0021_finance.sql") },
    Migration {
        version: 22,
        name: "finance_staff",
        sql: include_str!("../migrations/0022_finance_staff.sql"),
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

/// SQLCipher page size of a new clinic database (docs/design/db-size-rules.md). Every table and
/// every index takes at least one whole page even when empty, so with ~250 b-trees the page size,
/// not the data, decided the size of a small clinic: an empty schema is 1.1 MB with 4096-byte pages
/// and 350 KB with 1024-byte pages. SQLCipher does not store the page size in the (encrypted)
/// header, so it is set on every connection.
pub const PAGE_SIZE: i64 = 1024;
/// Page size of a big clinic database, and of every database made before v0.12.5 (the SQLCipher 4
/// default). Rows of 300-600 bytes pack better into big pages: measured per 100 patients with their
/// visit, treatment, invoice and payment, 939 KB at 1024 bytes and 788 KB at 4096 bytes.
pub const LARGE_PAGE_SIZE: i64 = 4096;
/// A small-page database that reaches this size is rewritten with [`LARGE_PAGE_SIZE`] at the next
/// start: the measured crossover (the same data smaller with big pages) is at about 4.9 MB.
pub const GROW_AT_BYTES: u64 = 5 * 1024 * 1024;
/// A big-page database under this size is rewritten with [`PAGE_SIZE`] (an old v0.12.4 test
/// database, or a big clinic restored from a small backup). The gap to [`GROW_AT_BYTES`] keeps a
/// database from switching back and forth.
pub const SHRINK_BELOW_BYTES: u64 = 4 * 1024 * 1024;
/// The WAL file is cut back to this size after a checkpoint (it otherwise keeps its largest size).
pub const JOURNAL_SIZE_LIMIT: i64 = 512 * 1024;

/// The page size a database of `file_size` bytes written with `current` should have.
pub fn target_page_size(current: i64, file_size: u64) -> i64 {
    match current {
        PAGE_SIZE if file_size >= GROW_AT_BYTES => LARGE_PAGE_SIZE,
        LARGE_PAGE_SIZE if file_size < SHRINK_BELOW_BYTES => PAGE_SIZE,
        other => other,
    }
}

/// Opens `path` with `key` at one page size; `WrongKey` when the file does not decrypt that way.
fn open_keyed(path: &Path, key: &DataKey, page_size: i64, flags: OpenFlags) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, flags)?;
    conn.execute_batch(&format!(
        "PRAGMA key = {}; PRAGMA cipher_page_size = {page_size};",
        key.sqlcipher_pragma_value()
    ))?;
    // Reading the schema is what validates the key (and the page size).
    if conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).is_err() {
        return Err(CoreError::WrongKey);
    }
    Ok(conn)
}

/// Opens with [`PAGE_SIZE`] or [`LARGE_PAGE_SIZE`], whichever the file was written with. A new
/// (empty) file gets [`PAGE_SIZE`].
fn open_any(path: &Path, key: &DataKey, flags: OpenFlags) -> Result<Connection> {
    match open_keyed(path, key, PAGE_SIZE, flags) {
        Err(CoreError::WrongKey) => open_keyed(path, key, LARGE_PAGE_SIZE, flags),
        other => other,
    }
}

/// True when the encrypted database file opens with `key`, read-only (the file is never changed).
pub fn opens_with(path: &Path, key: &DataKey) -> bool {
    open_any(path, key, OpenFlags::SQLITE_OPEN_READ_ONLY).is_ok()
}

/// Opens a connection with the clinic key and the mandatory PRAGMAs.
pub fn connect(path: &Path, key: &DataKey) -> Result<Connection> {
    let conn = open_any(path, key, OpenFlags::default())?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "FULL")?; // NFR: zero loss of committed transactions
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "journal_size_limit", JOURNAL_SIZE_LIMIT)?;
    // Up to 64 MB of decrypted pages (the default is 2 MB; memory is taken only as pages are read).
    // Small pages mean more pages and HMAC checks per query: with the default cache the 100 000
    // patient name search took 256 ms (limit 200 ms, OF-016); with this one 63 ms.
    conn.pragma_update(None, "cache_size", -65536)?;
    Ok(conn)
}

/// The SQLCipher page size the connection uses.
pub fn page_size(conn: &Connection) -> Result<i64> {
    let v: String = conn.query_row("PRAGMA cipher_page_size", [], |r| r.get(0))?;
    v.trim().parse().map_err(|_| CoreError::Integrity(format!("unexpected cipher_page_size {v}")))
}

/// Moves the WAL into the database file and cuts the WAL file to zero bytes.
pub fn checkpoint(conn: &Connection) -> Result<()> {
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    Ok(())
}

/// `VACUUM`s when `always` or when more than an eighth of the file is free pages (after a migration
/// dropped tables). Returns whether it ran.
pub fn vacuum_if_sparse(conn: &Connection, always: bool) -> Result<bool> {
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let free: i64 = conn.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
    if free == 0 || !(always || free * 8 > pages) {
        return Ok(false);
    }
    conn.execute_batch("VACUUM;")?;
    Ok(true)
}

/// Gives the database at `path` the page size its size calls for ([`target_page_size`]), rewriting
/// it in place when needed. Nothing may have the file open. Returns `true` when it rewrote.
pub fn fit_page_size(path: &Path, key: &DataKey) -> Result<bool> {
    let old = path.with_extension("repage-old");
    if !path.exists() && old.exists() {
        // A crash came between the two renames of [`repage`]: the old file is complete.
        fs::rename(&old, path)?;
    }
    remove_with_journals(&old);
    let conn = connect(path, key)?;
    checkpoint(&conn)?;
    let current = page_size(&conn)?;
    drop(conn);
    let target = target_page_size(current, fs::metadata(path)?.len());
    if target == current {
        return Ok(false);
    }
    repage(path, key, target)?;
    Ok(true)
}

/// Rewrites the database at `path` with `page_size_to` (`sqlcipher_export`, same key), in place.
/// Nothing may have the file open.
///
/// Crash-safe: the new file is written beside the old one and checked (integrity, the same row
/// count in every table, the same schema version) before it replaces it; the old file is kept as
/// `clinic.repage-old` until the new one is in place, and put back on the next start if a crash
/// came between the two renames.
pub fn repage(path: &Path, key: &DataKey, page_size_to: i64) -> Result<()> {
    let new = path.with_extension("repage");
    let old = path.with_extension("repage-old");
    remove_with_journals(&new);
    let conn = connect(path, key)?;
    checkpoint(&conn)?;
    let result = (|| -> Result<()> {
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        conn.execute(
            "ATTACH DATABASE ?1 AS repage KEY ?2",
            params![new.to_string_lossy(), format!("x'{}'", hex::encode(key.as_bytes()))],
        )?;
        conn.execute_batch(&format!("PRAGMA repage.cipher_page_size = {page_size_to};"))?;
        conn.query_row("SELECT sqlcipher_export('repage')", [], |_| Ok(()))?;
        conn.execute_batch(&format!("PRAGMA repage.user_version = {version};"))?;
        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM main.sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
                 AND sql NOT LIKE 'CREATE VIRTUAL%'",
            )?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for t in &tables {
            let count = |db: &str| -> Result<i64> {
                Ok(conn.query_row(&format!("SELECT count(*) FROM {db}.\"{t}\""), [], |r| r.get(0))?)
            };
            if count("main")? != count("repage")? {
                return Err(CoreError::Integrity(format!("rewritten copy differs in table {t}")));
            }
        }
        conn.execute_batch("DETACH DATABASE repage;")?;
        let check = connect(&new, key)?;
        if page_size(&check)? != page_size_to || schema_version(&check)? != schema_version(&conn)? {
            return Err(CoreError::Integrity("rewritten copy has the wrong layout".into()));
        }
        full_integrity_check(&check)?;
        checkpoint(&check)?;
        Ok(())
    })();
    drop(conn);
    if let Err(e) = result {
        remove_with_journals(&new);
        return Err(e);
    }
    remove_with_journals(&old);
    fs::rename(path, &old)?;
    remove_journals(path);
    if let Err(e) = fs::rename(&new, path) {
        let _ = fs::rename(&old, path);
        return Err(e.into());
    }
    remove_journals(&new);
    remove_with_journals(&old);
    tracing::info!(page_size = page_size_to, "database rewritten with another page size");
    Ok(())
}

fn journal(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", path.display()))
}

fn remove_journals(path: &Path) {
    for s in ["-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(journal(path, s));
    }
}

fn remove_with_journals(path: &Path) {
    let _ = fs::remove_file(path);
    remove_journals(path);
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
        prune_pre_migration(backup_dir, PRE_MIGRATION_KEPT);
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

/// Safety copies made before migrations that are kept. They are not in the backup register, so
/// retention never saw them and they piled up, one full database per update.
pub const PRE_MIGRATION_KEPT: usize = 2;

/// Removes all but the newest `keep` `pre-migration-v*-to-v*-<unix time>.db` files in `dir`.
pub fn prune_pre_migration(dir: &Path, keep: usize) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut found: Vec<(i64, PathBuf)> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let stamp = name.strip_prefix("pre-migration-")?.strip_suffix(".db")?.rsplit('-').next()?;
            Some((stamp.parse().ok()?, e.path()))
        })
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, p) in found.into_iter().skip(keep) {
        remove_with_journals(&p);
    }
}

/// Consistent, compact encrypted snapshot (same key, same page size) that does not block writers.
///
/// Not `VACUUM INTO`: SQLCipher writes its target with the *default* page settings, so with
/// [`PAGE_SIZE`] the copy could not be opened again. The copy is an attached database with the
/// connection's key and page size, filled by `sqlcipher_export` inside one read transaction (one
/// snapshot of the source while other connections keep writing).
pub fn vacuum_into(conn: &Connection, dest: &Path) -> Result<()> {
    remove_with_journals(dest);
    let page = page_size(conn)?;
    conn.execute("ATTACH DATABASE ?1 AS snapshot", [dest.to_string_lossy()])?;
    let result = (|| -> Result<()> {
        conn.execute_batch(&format!("PRAGMA snapshot.cipher_page_size = {page}; BEGIN;"))?;
        let copied = (|| -> Result<()> {
            let version: i64 = conn.query_row("PRAGMA main.user_version", [], |r| r.get(0))?;
            conn.query_row("SELECT sqlcipher_export('snapshot')", [], |_| Ok(()))?;
            conn.execute_batch(&format!("PRAGMA snapshot.user_version = {version};"))?;
            Ok(())
        })();
        match copied {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
        Ok(())
    })();
    let detached = conn.execute_batch("DETACH DATABASE snapshot;");
    if let Err(e) = result {
        remove_with_journals(dest);
        return Err(e);
    }
    detached?;
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
