//! Reading a clinic database from outside the program (support and the owner's own scripts).
//!
//! The database is encrypted (ADR-03), so an ordinary SQLite tool cannot open it. This module opens a
//! *backup* with the Recovery Key printed at set-up (ADR-04) and writes either a plain SQL script or an
//! unencrypted SQLite file. Both contain every patient's data: whoever runs it owns the output.

use std::fs;
use std::io::Write;
use std::path::Path;

use rusqlite::Connection;

use crate::db;
use crate::error::{CoreError, Result};
use crate::keys::{DataKey, RecoveryKey};

/// What a dump wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpReport {
    pub tables: usize,
    pub rows: i64,
    /// Search-index tables and the triggers that feed them are not part of a dump (the program
    /// rebuilds them), listed here so nothing disappears silently.
    pub skipped: Vec<String>,
}

/// The data key of a backup: its `.rkey` file opened with the Recovery Key.
pub fn key_from_recovery(rkey_file: &Path, recovery_key: &str) -> Result<DataKey> {
    let wrapped = fs::read(rkey_file)?;
    let key = RecoveryKey::parse(recovery_key)?;
    Ok(key.unwrap(&wrapped)?)
}

/// Opens a *copy* of the backup in `work_dir` (the original is never touched; opening a database
/// changes its journal files). The caller removes `work_dir`.
pub fn open_copy(backup: &Path, key: &DataKey, work_dir: &Path) -> Result<Connection> {
    fs::create_dir_all(work_dir)?;
    let copy = work_dir.join("dump-source.db");
    fs::copy(backup, &copy)?;
    db::connect(&copy, key)
}

fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn query_pairs(conn: &Connection, sql: &str) -> Result<Vec<(String, Option<String>, String)>> {
    let mut st = conn.prepare(sql)?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Writes the whole database as SQL: tables with their rows, then indexes, views and triggers.
pub fn write_sql(conn: &Connection, out: &mut impl Write) -> Result<DumpReport> {
    let objects = query_pairs(
        conn,
        "SELECT type, sql, name || char(31) || tbl_name FROM sqlite_master
         WHERE name NOT LIKE 'sqlite_%' ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'index' THEN 1 ELSE 2 END, name",
    )?;
    // Search indexes (FTS5) are virtual tables with shadow tables; they are rebuilt by the program.
    let virtual_tables: Vec<String> = objects
        .iter()
        .filter(|(kind, sql, _)| {
            kind == "table" && sql.as_deref().is_some_and(|s| s.to_uppercase().starts_with("CREATE VIRTUAL"))
        })
        .filter_map(|(_, _, names)| names.split('\u{1f}').next().map(str::to_string))
        .collect();
    let is_search_part = |name: &str| {
        virtual_tables.iter().any(|v| name == v || name.strip_prefix(v.as_str()).is_some_and(|r| r.starts_with('_')))
    };

    writeln!(out, "-- Artaveo Dental database dump. Contains patient data: keep it safe, delete it after use.")?;
    writeln!(out, "-- Search-index (FTS) tables are not included; the program rebuilds them.")?;
    writeln!(out, "PRAGMA foreign_keys=OFF;\nBEGIN TRANSACTION;")?;

    let mut report = DumpReport { tables: 0, rows: 0, skipped: Vec::new() };
    let mut later: Vec<String> = Vec::new();
    for (kind, sql, names) in &objects {
        let mut parts = names.split('\u{1f}');
        let (name, table) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let Some(sql) = sql else { continue };
        if is_search_part(name) || is_search_part(table) || (kind != "table" && virtual_tables.iter().any(|v| sql.contains(v.as_str()))) {
            report.skipped.push(format!("{kind} {name}"));
            continue;
        }
        if kind != "table" {
            later.push(format!("{sql};"));
            continue;
        }
        writeln!(out, "{sql};")?;
        report.tables += 1;
        let columns: Vec<String> = {
            let mut st = conn.prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")?;
            let rows = st.query_map([name], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        if columns.is_empty() {
            continue;
        }
        let quoted = columns.iter().map(|c| format!("quote({})", ident(c))).collect::<Vec<_>>().join(", ");
        let names_list = columns.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
        let mut st = conn.prepare(&format!("SELECT {quoted} FROM {}", ident(name)))?;
        let mut rows = st.query([])?;
        while let Some(row) = rows.next()? {
            let mut values = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                values.push(row.get::<_, String>(i)?);
            }
            writeln!(out, "INSERT INTO {}({names_list}) VALUES({});", ident(name), values.join(","))?;
            report.rows += 1;
        }
    }
    for statement in later {
        writeln!(out, "{statement}")?;
    }
    writeln!(out, "COMMIT;")?;
    Ok(report)
}

/// Writes an unencrypted SQLite copy of the database (SQLCipher's own export).
pub fn export_plain_sqlite(conn: &Connection, out: &Path) -> Result<()> {
    if out.exists() {
        return Err(CoreError::validation("the output file already exists"));
    }
    let target = out.display().to_string().replace('\'', "''");
    conn.execute_batch(&format!("ATTACH DATABASE '{target}' AS plain KEY '';"))?;
    let exported = conn.query_row("SELECT sqlcipher_export('plain')", [], |_| Ok(()));
    let detached = conn.execute_batch("DETACH DATABASE plain;");
    exported?;
    detached?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::default_protector;

    /// A small encrypted clinic-like database with every kind of value a dump must carry.
    fn sample() -> (tempfile::TempDir, std::path::PathBuf, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clinic.db");
        let recovery = RecoveryKey::generate();
        let (conn, _key) = db::create(&path, default_protector().as_ref(), &recovery).unwrap();
        conn.execute_batch(
            "CREATE TABLE patient (id TEXT PRIMARY KEY, name TEXT, note TEXT, photo BLOB, balance INTEGER, ratio REAL) STRICT;
             CREATE INDEX patient_name ON patient(name);
             CREATE VIRTUAL TABLE patient_fts USING fts5(name);
             CREATE TRIGGER patient_ai AFTER INSERT ON patient BEGIN INSERT INTO patient_fts(name) VALUES (new.name); END;
             INSERT INTO patient VALUES ('p1', 'O''Brien', 'line1' || char(10) || 'line2', x'00ff10', 12345, 0.5);
             INSERT INTO patient VALUES ('p2', 'احمد', NULL, NULL, -7, NULL);
             PRAGMA wal_checkpoint(TRUNCATE);",
        )
        .unwrap();
        drop(conn);
        (dir, path, recovery.to_display())
    }

    #[test]
    fn a_backup_opens_with_the_recovery_key_and_dumps_to_sql_that_loads_back_the_same() {
        let (dir, path, recovery) = sample();
        let key = key_from_recovery(&path.with_extension("rkey"), &recovery).unwrap();
        let conn = open_copy(&path, &key, &dir.path().join("work")).unwrap();
        let mut sql = Vec::new();
        let report = write_sql(&conn, &mut sql).unwrap();
        let text = String::from_utf8(sql).unwrap();

        assert_eq!(report.tables, 1);
        assert_eq!(report.rows, 2);
        assert!(text.contains("'O''Brien'") && text.contains("X'00FF10'") && text.contains("'احمد'"));
        assert!(!text.contains("patient_fts_"), "no search-index shadow tables");
        assert!(report.skipped.iter().any(|s| s == "table patient_fts" || s == "trigger patient_ai"));

        // The script runs in a plain SQLite and gives the same rows back.
        let plain = Connection::open_in_memory().unwrap();
        plain.execute_batch(&text).unwrap();
        let (name, note, balance): (String, String, i64) = plain
            .query_row("SELECT name, note, balance FROM patient WHERE id = 'p1'", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((name.as_str(), note.as_str(), balance), ("O'Brien", "line1\nline2", 12345));
        let blob: Vec<u8> = plain.query_row("SELECT photo FROM patient WHERE id = 'p1'", [], |r| r.get(0)).unwrap();
        assert_eq!(blob, vec![0x00, 0xff, 0x10]);
        let null_note: Option<String> =
            plain.query_row("SELECT note FROM patient WHERE id = 'p2'", [], |r| r.get(0)).unwrap();
        assert!(null_note.is_none());
    }

    #[test]
    fn a_wrong_recovery_key_is_refused() {
        let (_dir, path, _recovery) = sample();
        let other = RecoveryKey::generate().to_display();
        assert!(key_from_recovery(&path.with_extension("rkey"), &other).is_err());
        assert!(key_from_recovery(&path.with_extension("rkey"), "not a key").is_err());
    }

    #[test]
    fn the_plain_sqlite_export_opens_without_a_key_and_never_overwrites() {
        let (dir, path, recovery) = sample();
        let key = key_from_recovery(&path.with_extension("rkey"), &recovery).unwrap();
        let conn = open_copy(&path, &key, &dir.path().join("work")).unwrap();
        let out = dir.path().join("plain.sqlite");
        export_plain_sqlite(&conn, &out).unwrap();
        let plain = Connection::open(&out).unwrap();
        let n: i64 = plain.query_row("SELECT COUNT(*) FROM patient", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
        assert!(export_plain_sqlite(&conn, &out).is_err(), "an existing file is never overwritten");
    }
}
