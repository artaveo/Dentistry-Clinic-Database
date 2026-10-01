//! Clinic settings stored in the encrypted DB (audited on change).

use artaveo_shared::{Settings, ValidationRule};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::clock::now_iso;
use crate::error::{CoreError, Result};

pub const DEFAULTS: Settings =
    Settings { session_timeout_minutes: 10, daily_backup_hour: 19, backup_keep_daily: 14 };

fn read(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

fn num(conn: &Connection, key: &str, default: u32) -> Result<u32> {
    Ok(read(conn, key)?.and_then(|v| v.parse().ok()).unwrap_or(default))
}

pub fn get(conn: &Connection) -> Result<Settings> {
    Ok(Settings {
        session_timeout_minutes: num(conn, "session_timeout_minutes", DEFAULTS.session_timeout_minutes)?,
        daily_backup_hour: num(conn, "daily_backup_hour", DEFAULTS.daily_backup_hour)?,
        backup_keep_daily: num(conn, "backup_keep_daily", DEFAULTS.backup_keep_daily)?,
    })
}

pub fn update(conn: &Connection, actor: &Actor, new: &Settings) -> Result<Settings> {
    if !(1..=240).contains(&new.session_timeout_minutes) {
        return Err(CoreError::invalid(
            "session_timeout_minutes",
            ValidationRule::SessionTimeoutRange,
            "session timeout must be 1–240 minutes",
        ));
    }
    if new.daily_backup_hour > 23 {
        return Err(CoreError::invalid(
            "daily_backup_hour",
            ValidationRule::BackupHourRange,
            "backup hour must be 0–23",
        ));
    }
    if !(1..=365).contains(&new.backup_keep_daily) {
        return Err(CoreError::invalid(
            "backup_keep_daily",
            ValidationRule::BackupKeepRange,
            "backups to keep must be 1–365",
        ));
    }
    let before = get(conn)?;
    let now = now_iso();
    for (k, v) in [
        ("session_timeout_minutes", new.session_timeout_minutes),
        ("daily_backup_hour", new.daily_backup_hour),
        ("backup_keep_daily", new.backup_keep_daily),
    ] {
        conn.execute(
            "INSERT INTO setting(key, value, updated_at, updated_by) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at, updated_by = excluded.updated_by",
            params![k, v.to_string(), now, actor.user_id],
        )?;
    }
    let after = get(conn)?;
    audit::record(
        conn,
        actor,
        "settings.update",
        Some("setting"),
        None,
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}
