//! Append-only audit log (roadmap 1.6). The table rejects UPDATE/DELETE via
//! triggers; there is no API to modify or delete entries.

use std::sync::OnceLock;

use artaveo_shared::{AuditEntry, AuditListParams};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection};
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

/// Actions that deserve a second look (roadmap 9.8): deleting, voiding, refunding, changing who may do
/// what, recovery, restore and licence events.
const SENSITIVE_EXACT: &[&str] = &[
    "patient.update",
    "patient.merge",
    "invoice.credit_note",
    "user.update",
    "user.reset_password",
    "user.unlock",
    "auth.recover_owner",
    "auth.recover_owner_failed",
    "chart.finding_remove",
    "stock.adjust",
    "lan.revoke",
    "billing.settings",
    "commission.rule",
];
/// SQL `LIKE` patterns (`%` only at the ends), mirrored by [`is_sensitive`].
const SENSITIVE_LIKE: &[&str] = &["%delete%", "%void%", "%refund%", "role.%", "license.%", "%restore%", "%override%"];

/// Whether an audit action is one of the sensitive ones.
pub fn is_sensitive(action: &str) -> bool {
    SENSITIVE_EXACT.contains(&action)
        || SENSITIVE_LIKE.iter().any(|p| {
            let core = p.trim_matches('%');
            match (p.starts_with('%'), p.ends_with('%')) {
                (true, true) => action.contains(core),
                (true, false) => action.ends_with(core),
                (false, true) => action.starts_with(core),
                (false, false) => action == core,
            }
        })
}

/// Escapes `%`, `_` and `\` so user text is matched literally by `LIKE … ESCAPE '\'`.
fn like_literal(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// Newest first. `entity_id` narrows to one record's history (e.g. a
/// patient's "Audit History" tab, 3.6); `None` is the global log, which the viewer (9.8) can filter.
pub fn list(conn: &Connection, p: &AuditListParams) -> Result<Vec<AuditEntry>> {
    let mut sql = String::from(
        "SELECT id, at, user_id, username, action, entity, entity_id, old_value, new_value, computer
         FROM audit_log WHERE 1 = 1",
    );
    let mut args: Vec<Value> = Vec::new();
    let clean = |s: &Option<String>| s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    if let Some(id) = clean(&p.entity_id) {
        sql.push_str(" AND entity_id = ?");
        args.push(Value::Text(id));
    }
    if let Some(a) = clean(&p.action) {
        sql.push_str(" AND action LIKE ? ESCAPE '\\'");
        args.push(Value::Text(format!("%{}%", like_literal(&a))));
    }
    if let Some(u) = clean(&p.username) {
        sql.push_str(" AND username = ?");
        args.push(Value::Text(u));
    }
    if clean(&p.from).is_some() || clean(&p.to).is_some() {
        let from = clean(&p.from).unwrap_or_else(|| "2000-01-01".into());
        let to = clean(&p.to).unwrap_or_else(|| "2100-01-01".into());
        let (lo, hi) = crate::reports::utc_bounds(&from, &to)?;
        sql.push_str(" AND at >= ? AND at < ?");
        args.push(Value::Text(lo));
        args.push(Value::Text(hi));
    }
    if p.sensitive_only {
        let exact = SENSITIVE_EXACT.iter().map(|a| format!("'{a}'")).collect::<Vec<_>>().join(", ");
        let likes = SENSITIVE_LIKE.iter().map(|l| format!("action LIKE '{l}'")).collect::<Vec<_>>().join(" OR ");
        sql.push_str(&format!(" AND (action IN ({exact}) OR {likes})"));
    }
    sql.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
    args.push(Value::Integer(i64::from(p.limit.min(500))));
    args.push(Value::Integer(i64::from(p.offset)));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
        let action: String = r.get(4)?;
        Ok(AuditEntry {
            id: r.get(0)?,
            at: r.get(1)?,
            user_id: r.get(2)?,
            username: r.get(3)?,
            sensitive: is_sensitive(&action),
            action,
            entity: r.get(5)?,
            entity_id: r.get(6)?,
            old_value: r.get(7)?,
            new_value: r.get(8)?,
            computer: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
