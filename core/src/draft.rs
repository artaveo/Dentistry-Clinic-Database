//! Form drafts (OF-020, roadmap rule 14): what a user typed into a long form, kept until the form is
//! saved or cancelled. Each user has at most one draft per form; the values are opaque JSON that the
//! UI owns, so a new field never needs a migration.

use artaveo_shared::{DraftInfo, DraftParams, SaveDraftParams, ValidationRule};
use rusqlite::{params, Connection, OptionalExtension};

use crate::clock::now_iso;
use crate::error::{CoreError, Result};

/// A draft is a form, not a file: anything larger is a mistake or an attack.
const MAX_DRAFT_BYTES: usize = 64 * 1024;
const MAX_KEY_LEN: usize = 64;

fn check_key(form_key: &str) -> Result<()> {
    let ok = !form_key.is_empty()
        && form_key.len() <= MAX_KEY_LEN
        && form_key.chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '_' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(CoreError::invalid("form_key", ValidationRule::InvalidParams, "a draft needs a short form name"))
    }
}

pub fn get(conn: &Connection, user_id: &str, p: &DraftParams) -> Result<DraftInfo> {
    check_key(&p.form_key)?;
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT data_json, updated_at FROM form_draft WHERE user_id = ?1 AND form_key = ?2",
            params![user_id, p.form_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(match row {
        Some((data_json, updated_at)) => {
            DraftInfo { data_json: Some(data_json), updated_at: Some(updated_at) }
        }
        None => DraftInfo { data_json: None, updated_at: None },
    })
}

pub fn save(conn: &Connection, user_id: &str, p: &SaveDraftParams) -> Result<()> {
    check_key(&p.form_key)?;
    if p.data_json.len() > MAX_DRAFT_BYTES {
        return Err(CoreError::invalid("data_json", ValidationRule::InvalidParams, "the draft is too large"));
    }
    if !matches!(serde_json::from_str::<serde_json::Value>(&p.data_json), Ok(serde_json::Value::Object(_))) {
        return Err(CoreError::invalid(
            "data_json",
            ValidationRule::InvalidParams,
            "a draft must be a JSON object",
        ));
    }
    conn.execute(
        "INSERT INTO form_draft(user_id, form_key, data_json, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(user_id, form_key) DO UPDATE SET data_json = excluded.data_json, updated_at = excluded.updated_at",
        params![user_id, p.form_key, p.data_json, now_iso()],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, user_id: &str, p: &DraftParams) -> Result<()> {
    check_key(&p.form_key)?;
    conn.execute(
        "DELETE FROM form_draft WHERE user_id = ?1 AND form_key = ?2",
        params![user_id, p.form_key],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_short_plain_names() {
        assert!(check_key("patient.create").is_ok());
        assert!(check_key("appointment-new_1").is_ok());
        assert!(check_key("").is_err());
        assert!(check_key("Patient Form").is_err());
        assert!(check_key(&"a".repeat(65)).is_err());
    }
}
