//! Basic local backup (roadmap 1.7): manual and daily, encrypted, verified.
//! The full Backup Manager (USB, cloud folder, restore UI) is Phase 10.
//!
//! Each backup is `VACUUM INTO` from a *separate* connection (no lock on the
//! app's connection), re-opened and fully integrity-checked, and stored next
//! to a copy of the recovery-wrapped key (`.rkey`) so it can be restored on a
//! new computer with the Recovery Key alone (ADR-04).

use std::fs;
use std::path::Path;

use artaveo_shared::{BackupInfo, ErrorCode, Settings};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::audit::{self, Actor};
use crate::clock::{iso, now, parse, CLINIC_OFFSET};
use crate::config::Config;
use crate::db::{self, KeyFiles};
use crate::error::{CoreError, Result};
use crate::ids::new_id;
use crate::keys::DataKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Manual,
    Daily,
}

impl Kind {
    pub fn code(self) -> &'static str {
        match self {
            Kind::Manual => "manual",
            Kind::Daily => "daily",
        }
    }
}

fn map(r: &rusqlite::Row<'_>) -> rusqlite::Result<BackupInfo> {
    Ok(BackupInfo {
        id: r.get(0)?,
        file_name: r.get(1)?,
        created_at: r.get(2)?,
        size_bytes: r.get(3)?,
        kind: r.get(4)?,
        verified: r.get::<_, i64>(5)? == 1,
    })
}

const SELECT: &str =
    "SELECT id, file_name, created_at, size_bytes, kind, verified FROM backup_log WHERE deleted_at IS NULL";

pub fn list(conn: &Connection) -> Result<Vec<BackupInfo>> {
    let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY created_at DESC"))?;
    let v = stmt.query_map([], map)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn latest(conn: &Connection) -> Result<Option<BackupInfo>> {
    Ok(conn.query_row(&format!("{SELECT} ORDER BY created_at DESC LIMIT 1"), [], map).optional()?)
}

/// Creates, verifies and registers a backup. `conn` is the app connection
/// (used only to register/audit); the copy is made from a fresh connection.
pub fn create(
    conn: &Connection,
    key: &DataKey,
    config: &Config,
    kind: Kind,
    actor: &Actor,
) -> Result<BackupInfo> {
    let dir = config.backup_dir();
    fs::create_dir_all(&dir)?;
    let at = now();
    let local = at.to_offset(CLINIC_OFFSET);
    let file_name = format!(
        "artaveo-{:04}{:02}{:02}-{:02}{:02}{:02}-{}.db",
        local.year(),
        local.month() as u8,
        local.day(),
        local.hour(),
        local.minute(),
        local.second(),
        kind.code()
    );
    let path = dir.join(&file_name);

    let result = (|| -> Result<i64> {
        let source = db::connect(&config.db_path(), key)?;
        db::vacuum_into(&source, &path)?;
        drop(source);
        let copy = db::connect(&path, key)?;
        db::full_integrity_check(&copy)?;
        drop(copy);
        fs::copy(KeyFiles::beside(&config.db_path()).recovery, path.with_extension("rkey"))?;
        Ok(fs::metadata(&path)?.len() as i64)
    })();

    let size = match result {
        Ok(size) => size,
        Err(e) => {
            let _ = fs::remove_file(&path);
            let _ = fs::remove_file(path.with_extension("rkey"));
            audit::record(
                conn,
                actor,
                "backup.failed",
                Some("backup"),
                None,
                None,
                Some(&json!({"kind": kind.code(), "error": e.to_string()})),
            )?;
            tracing::error!(error = %e, "backup failed");
            return Err(CoreError::api(ErrorCode::Internal, format!("backup failed: {e}")));
        }
    };

    let info = BackupInfo {
        id: new_id(),
        file_name,
        created_at: iso(at),
        size_bytes: size,
        kind: kind.code().into(),
        verified: true,
    };
    conn.execute(
        "INSERT INTO backup_log(id, file_name, created_at, created_by, size_bytes, kind, verified)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
        params![info.id, info.file_name, info.created_at, actor.user_id, info.size_bytes, info.kind],
    )?;
    audit::record(conn, actor, "backup.create", Some("backup"), Some(&info.id), None, Some(&json!(info)))?;
    tracing::info!(file = %info.file_name, size = info.size_bytes, "backup created");
    Ok(info)
}

/// Keeps the newest `keep` backups; older files are deleted and marked.
pub fn apply_retention(conn: &Connection, dir: &Path, keep: u32) -> Result<u32> {
    let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY created_at DESC LIMIT -1 OFFSET ?1"))?;
    let old: Vec<BackupInfo> = stmt.query_map([keep], map)?.collect::<rusqlite::Result<_>>()?;
    for b in &old {
        let p = dir.join(&b.file_name);
        let _ = fs::remove_file(&p);
        let _ = fs::remove_file(p.with_extension("rkey"));
        conn.execute("UPDATE backup_log SET deleted_at = ?1 WHERE id = ?2", params![iso(now()), b.id])?;
    }
    Ok(old.len() as u32)
}

/// Daily rule: due when there is no backup in the last 24 h, or when the
/// configured local hour has passed and today has no backup yet.
pub fn is_due(latest: Option<&BackupInfo>, settings: &Settings, at: OffsetDateTime) -> bool {
    let Some(last) = latest.and_then(|b| parse(&b.created_at)) else { return true };
    if at - last >= Duration::hours(24) {
        return true;
    }
    let local_now = at.to_offset(CLINIC_OFFSET);
    let local_last = last.to_offset(CLINIC_OFFSET);
    local_now.hour() as u32 >= settings.daily_backup_hour && local_last.date() < local_now.date()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn b(at: &str) -> BackupInfo {
        BackupInfo {
            id: "x".into(),
            file_name: "f".into(),
            created_at: at.into(),
            size_bytes: 1,
            kind: "daily".into(),
            verified: true,
        }
    }

    #[test]
    fn daily_due_rules() {
        let s = crate::settings::DEFAULTS; // 19:00 Kabul
        assert!(is_due(None, &s, now()));
        // Yesterday 19:05 Kabul (14:35Z); now today 18:00 Kabul → not due (< 24 h, before 19:00).
        let last = b("2026-10-01T14:35:00.000Z");
        assert!(!is_due(Some(&last), &s, datetime!(2026-10-02 13:30 UTC)));
        // Today 19:10 Kabul → due.
        assert!(is_due(Some(&last), &s, datetime!(2026-10-02 14:40 UTC)));
        // Already backed up today at 19:01 Kabul → not due at 22:00.
        let today = b("2026-10-02T14:31:00.000Z");
        assert!(!is_due(Some(&today), &s, datetime!(2026-10-02 17:30 UTC)));
        // App closed for two days → due immediately in the morning.
        assert!(is_due(Some(&last), &s, datetime!(2026-10-03 04:00 UTC)));
    }
}
