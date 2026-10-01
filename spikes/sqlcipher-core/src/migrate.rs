use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::store::StoreError;

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
    Migration { version: 1, name: "init", sql: include_str!("../migrations/0001_init.sql") },
    Migration { version: 2, name: "invoice", sql: include_str!("../migrations/0002_invoice.sql") },
];

fn ensure_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            checksum   TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        ) STRICT;",
    )
}

pub fn current_version(conn: &Connection) -> Result<i64, StoreError> {
    ensure_table(conn)?;
    Ok(conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?)
}

/// Returns versions that still need to be applied, after verifying that
/// already-applied migrations were not edited (checksum drift).
pub fn pending<'a>(conn: &Connection, set: &'a [Migration]) -> Result<Vec<&'a Migration>, StoreError> {
    ensure_table(conn)?;
    let mut out = Vec::new();
    for m in set {
        let applied: Option<String> = conn
            .query_row("SELECT checksum FROM schema_migrations WHERE version = ?1", [m.version], |r| r.get(0))
            .optional()?;
        match applied {
            Some(sum) if sum != m.checksum() => {
                return Err(StoreError::MigrationDrift { version: m.version })
            }
            Some(_) => {}
            None => out.push(m),
        }
    }
    let known_max = set.iter().map(|m| m.version).max().unwrap_or(0);
    let db_max = current_version(conn)?;
    if db_max > known_max {
        return Err(StoreError::DatabaseNewerThanApp { db: db_max, app: known_max });
    }
    Ok(out)
}

/// Each migration runs in its own IMMEDIATE transaction; a failure rolls
/// back that migration only and stops.
pub fn apply(conn: &mut Connection, todo: &[&Migration]) -> Result<(), StoreError> {
    for m in todo {
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute_batch(m.sql).map_err(|e| StoreError::MigrationFailed { version: m.version, source: e })?;
        tx.execute(
            "INSERT INTO schema_migrations(version, name, checksum) VALUES (?1, ?2, ?3)",
            params![m.version, m.name, m.checksum()],
        )?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
    }
    Ok(())
}
