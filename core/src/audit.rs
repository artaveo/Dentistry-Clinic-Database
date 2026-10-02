//! Append-only audit log (roadmap 1.6). The table rejects UPDATE/DELETE via
//! triggers; there is no API to modify or delete entries.

use std::sync::OnceLock;

use artaveo_shared::AuditEntry;
use rusqlite::{params, Connection};
use serde_json::Value;

use crate::clock::now_iso;
use crate::error::Result;

/// Who performed an action. `None` user = the system itself (migrations,
/// scheduled backup) or an unauthenticated attempt (failed login).
#[derive(Debug, Clone, Default)]
pub struct Actor {
    pub user_id: Option<String>,
    pub username: Option<String>,
}

impl Actor {
    pub fn system() -> Self {
        Self::default()
    }
    pub fn user(id: &str, username: &str) -> Self {
        Self { user_id: Some(id.into()), username: Some(username.into()) }
    }
}

pub fn computer_name() -> &'static str {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| gethostname::gethostname().to_string_lossy().into_owned())
}

pub fn record(
    conn: &Connection,
    actor: &Actor,
    action: &str,
    entity: Option<&str>,
    entity_id: Option<&str>,
    old_value: Option<&Value>,
    new_value: Option<&Value>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_log(at, user_id, username, action, entity, entity_id, old_value, new_value, computer)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            now_iso(),
            actor.user_id,
            actor.username,
            action,
            entity,
            entity_id,
            old_value.map(Value::to_string),
            new_value.map(Value::to_string),
            computer_name(),
        ],
    )?;
    Ok(())
}

/// Newest first. `entity_id` narrows to one record's history (e.g. a
/// patient's "Audit History" tab, 3.6); `None` is the global log.
pub fn list(conn: &Connection, limit: u32, offset: u32, entity_id: Option<&str>) -> Result<Vec<AuditEntry>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, at, user_id, username, action, entity, entity_id, old_value, new_value, computer
         FROM audit_log WHERE ?3 IS NULL OR entity_id = ?3 ORDER BY id DESC LIMIT ?1 OFFSET ?2",
    )?;
    let rows = stmt.query_map(params![limit.min(500), offset, entity_id], |r| {
        Ok(AuditEntry {
            id: r.get(0)?,
            at: r.get(1)?,
            user_id: r.get(2)?,
            username: r.get(3)?,
            action: r.get(4)?,
            entity: r.get(5)?,
            entity_id: r.get(6)?,
            old_value: r.get(7)?,
            new_value: r.get(8)?,
            computer: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
