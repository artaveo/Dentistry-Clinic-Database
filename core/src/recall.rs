//! Follow-up & recall (roadmap 4.6). A recall says "this patient should come
//! back around `due_date`": a doctor's follow-up, a recurring check-up or
//! scaling every N months, or a patient who missed their visit. The reception
//! call list is simply the recalls that are still `pending` or `contacted`.
//!
//! Life cycle:
//!
//! ```text
//! pending ⇄ contacted ──(visit booked)──▶ booked ──(visit completed)──▶ done
//!    └──────────────────▶ dismissed          └─(visit cancelled)──▶ pending
//! ```
//!
//! `booked` and `done` follow the appointments (`appointment.rs`), never a
//! manual status change. A recurring recall (`repeat_months`) creates its
//! successor when its visit is completed.

use artaveo_shared::{
    CreateRecallParams, ErrorCode, FollowUpInput, RecallInfo, RecallKind, RecallListParams, RecallListResult,
    RecallStatus, SetRecallStatusParams, UpdateRecallParams, ValidationRule,
};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::json;
use time::{Date, Month};

use crate::audit::{self, Actor};
use crate::clock::now_iso;
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::new_id;
use crate::scheduling::{format_date, parse_date};

const RECALL_SELECT: &str =
    "SELECT r.id, r.patient_id, p.patient_number, p.full_name, p.phone, r.kind, r.due_date,
        r.repeat_months, r.note, r.status, r.appointment_id, r.source_appointment_id,
        r.last_contacted_at, r.contact_note, r.version
     FROM recall r JOIN patient p ON p.id = r.patient_id WHERE r.deleted_at IS NULL";

fn map_recall(r: &rusqlite::Row<'_>) -> rusqlite::Result<RecallInfo> {
    Ok(RecallInfo {
        id: r.get(0)?,
        patient_id: r.get(1)?,
        patient_number: r.get(2)?,
        patient_name: r.get(3)?,
        patient_phone: r.get(4)?,
        kind: RecallKind::from_code(&r.get::<_, String>(5)?).unwrap_or(RecallKind::Other),
        due_date: r.get(6)?,
        repeat_months: r.get(7)?,
        note: r.get(8)?,
        status: RecallStatus::from_code(&r.get::<_, String>(9)?).unwrap_or(RecallStatus::Pending),
        appointment_id: r.get(10)?,
        source_appointment_id: r.get(11)?,
        last_contacted_at: r.get(12)?,
        contact_note: r.get(13)?,
        version: r.get(14)?,
    })
}

pub fn get_recall(conn: &Connection, id: &str) -> Result<RecallInfo> {
    conn.query_row(&format!("{RECALL_SELECT} AND r.id = ?1"), [id], map_recall).optional()?.ok_or_else(|| {
        CoreError::api(ErrorCode::NotFound, format!("recall {id}"))
            .on_field("id", ValidationRule::RecallNotFound)
    })
}

/// `n` months after `d`, clamping the day to the target month's length
/// (31 January + 1 month = 28/29 February).
pub(crate) fn add_months(d: Date, n: i64) -> Date {
    let total = d.year() as i64 * 12 + (d.month() as i64 - 1) + n;
    let year = total.div_euclid(12) as i32;
    let month = Month::try_from((total.rem_euclid(12) + 1) as u8).expect("1..=12");
    let day = d.day().min(month.length(year));
    Date::from_calendar_date(year, month, day).expect("day was clamped to the month length")
}

fn validate(due_date: &str, repeat_months: Option<i64>, note: Option<&str>) -> Result<Date> {
    let due = parse_date("due_date", due_date)?;
    if repeat_months.is_some_and(|m| !(1..=60).contains(&m)) {
        return Err(CoreError::invalid(
            "repeat_months",
            ValidationRule::RepeatMonthsRange,
            "repeat interval must be 1-60 months",
        ));
    }
    if note.is_some_and(|n| n.chars().count() > 500) {
        return Err(CoreError::invalid(
            "note",
            ValidationRule::Required,
            "note must be at most 500 characters",
        ));
    }
    Ok(due)
}

fn ensure_patient(conn: &Connection, patient_id: &str) -> Result<()> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT merged_into_id FROM patient WHERE id = ?1 AND deleted_at IS NULL",
            [patient_id],
            |r| r.get(0),
        )
        .optional()?;
    match row {
        None => Err(CoreError::api(ErrorCode::NotFound, format!("patient {patient_id}"))
            .on_field("patient_id", ValidationRule::PatientNotFound)),
        Some(Some(_)) => Err(CoreError::invalid(
            "patient_id",
            ValidationRule::PatientMerged,
            "this record was merged into another patient",
        )),
        Some(None) => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn insert(
    conn: &Connection,
    actor: &Actor,
    patient_id: &str,
    kind: RecallKind,
    due_date: &str,
    repeat_months: Option<i64>,
    note: Option<&str>,
    source_appointment_id: Option<&str>,
) -> Result<RecallInfo> {
    let id = new_id();
    let now = now_iso();
    conn.execute(
        "INSERT INTO recall(id, patient_id, kind, due_date, repeat_months, note, source_appointment_id,
                            created_at, created_by, updated_at, updated_by)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?8,?9)",
        params![
            id,
            patient_id,
            kind.code(),
            due_date,
            repeat_months,
            note.map(str::trim).filter(|s| !s.is_empty()),
            source_appointment_id,
            now,
            actor.user_id
        ],
    )?;
    let info = get_recall(conn, &id)?;
    audit::record(conn, actor, "recall.create", Some("recall"), Some(&id), None, Some(&json!(info)))?;
    Ok(info)
}

pub fn create_recall(conn: &Connection, actor: &Actor, p: &CreateRecallParams) -> Result<RecallInfo> {
    let due = validate(&p.due_date, p.repeat_months, p.note.as_deref())?;
    ensure_patient(conn, &p.patient_id)?;
    insert(conn, actor, &p.patient_id, p.kind, &format_date(due), p.repeat_months, p.note.as_deref(), None)
}

/// A follow-up chosen while completing a visit (4.6).
pub(crate) fn create_follow_up(
    conn: &Connection,
    actor: &Actor,
    patient_id: &str,
    source_appointment_id: &str,
    f: &FollowUpInput,
) -> Result<RecallInfo> {
    let due =
        validate(&f.due_date, f.repeat_months, f.note.as_deref()).map_err(|e| e.rename_field("follow_up"))?;
    insert(
        conn,
        actor,
        patient_id,
        f.kind,
        &format_date(due),
        f.repeat_months,
        f.note.as_deref(),
        Some(source_appointment_id),
    )
}

pub fn update_recall(conn: &Connection, actor: &Actor, p: &UpdateRecallParams) -> Result<RecallInfo> {
    let due = validate(&p.due_date, p.repeat_months, p.note.as_deref())?;
    let before = get_recall(conn, &p.id)?;
    if !matches!(before.status, RecallStatus::Pending | RecallStatus::Contacted) {
        return Err(CoreError::invalid("id", ValidationRule::NotEditable, "only open recalls can be edited"));
    }
    let changed = conn.execute(
        "UPDATE recall SET kind=?1, due_date=?2, repeat_months=?3, note=?4, updated_at=?5, updated_by=?6, version=version+1
         WHERE id=?7 AND version=?8 AND deleted_at IS NULL",
        params![
            p.kind.code(),
            format_date(due),
            p.repeat_months,
            p.note.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            now_iso(),
            actor.user_id,
            p.id,
            p.version
        ],
    )?;
    expect_one_row(changed, "recall")?;
    let after = get_recall(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        "recall.update",
        Some("recall"),
        Some(&p.id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

/// Manual moves only: record a call (`contacted`), put it back (`pending`) or `dismissed`.
pub fn set_recall_status(conn: &Connection, actor: &Actor, p: &SetRecallStatusParams) -> Result<RecallInfo> {
    let before = get_recall(conn, &p.id)?;
    let manual =
        matches!(p.status, RecallStatus::Pending | RecallStatus::Contacted | RecallStatus::Dismissed);
    let editable =
        matches!(before.status, RecallStatus::Pending | RecallStatus::Contacted | RecallStatus::Dismissed);
    if !manual || !editable {
        return Err(CoreError::invalid(
            "status",
            ValidationRule::InvalidTransition,
            format!("cannot move a {} recall to {}", before.status.code(), p.status.code()),
        ));
    }
    if p.note.as_deref().is_some_and(|n| n.chars().count() > 500) {
        return Err(CoreError::invalid(
            "note",
            ValidationRule::Required,
            "note must be at most 500 characters",
        ));
    }
    let now = now_iso();
    let (contacted_at, contact_note) = if p.status == RecallStatus::Contacted {
        (Some(now.clone()), p.note.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string))
    } else {
        (before.last_contacted_at.clone(), before.contact_note.clone())
    };
    let changed = conn.execute(
        "UPDATE recall SET status=?1, last_contacted_at=?2, contact_note=?3, updated_at=?4, updated_by=?5, version=version+1
         WHERE id=?6 AND version=?7 AND deleted_at IS NULL",
        params![p.status.code(), contacted_at, contact_note, now, actor.user_id, p.id, p.version],
    )?;
    expect_one_row(changed, "recall")?;
    let after = get_recall(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        "recall.status",
        Some("recall"),
        Some(&p.id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

/// The call list (default: pending + contacted) or any filtered view of recalls.
pub fn list_recalls(conn: &Connection, p: &RecallListParams) -> Result<RecallListResult> {
    let mut sql = String::from(" ");
    let mut args: Vec<Value> = Vec::new();
    if let Some(pid) = p.patient_id.as_deref() {
        sql.push_str("AND r.patient_id = ? ");
        args.push(pid.to_string().into());
    }
    let statuses: Vec<&str> = if p.statuses.is_empty() {
        vec!["pending", "contacted"]
    } else {
        p.statuses.iter().map(|s| s.code()).collect()
    };
    sql.push_str(&format!("AND r.status IN ({}) ", vec!["?"; statuses.len()].join(",")));
    args.extend(statuses.iter().map(|s| Value::from(s.to_string())));
    if let Some(d) = p.due_from.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str("AND r.due_date >= ? ");
        args.push(format_date(parse_date("due_from", d)?).into());
    }
    if let Some(d) = p.due_until.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str("AND r.due_date <= ? ");
        args.push(format_date(parse_date("due_until", d)?).into());
    }
    let total: u32 = conn.query_row(
        &format!("SELECT COUNT(*) FROM recall r WHERE r.deleted_at IS NULL {sql}"),
        params_from_iter(args.iter()),
        |r| r.get(0),
    )?;
    let limit = p.limit.clamp(1, 500);
    let mut stmt = conn.prepare(&format!(
        "{RECALL_SELECT} {sql} ORDER BY r.due_date, p.full_name COLLATE NOCASE LIMIT {limit} OFFSET {}",
        p.offset
    ))?;
    let items =
        stmt.query_map(params_from_iter(args.iter()), map_recall)?.collect::<rusqlite::Result<_>>()?;
    Ok(RecallListResult { items, total })
}

// ───────────────────────────── driven by appointments ─────────────────────────────

/// A visit was booked for this recall: it leaves the call list.
pub(crate) fn link_booking(
    conn: &Connection,
    actor: &Actor,
    recall_id: &str,
    appointment_id: &str,
    patient_id: &str,
) -> Result<()> {
    let before = get_recall(conn, recall_id)?;
    if before.patient_id != patient_id {
        return Err(CoreError::invalid(
            "recall_id",
            ValidationRule::RecallNotFound,
            "recall belongs to another patient",
        ));
    }
    if !matches!(before.status, RecallStatus::Pending | RecallStatus::Contacted) {
        return Err(CoreError::invalid(
            "recall_id",
            ValidationRule::NotEditable,
            "this recall is no longer open",
        ));
    }
    conn.execute(
        "UPDATE recall SET status='booked', appointment_id=?1, updated_at=?2, updated_by=?3, version=version+1 WHERE id=?4",
        params![appointment_id, now_iso(), actor.user_id, recall_id],
    )?;
    let after = get_recall(conn, recall_id)?;
    audit::record(
        conn,
        actor,
        "recall.booked",
        Some("recall"),
        Some(recall_id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(())
}

/// The booked visit was cancelled (or the patient did not come): the patient needs a call again.
pub(crate) fn reopen_for_appointment(conn: &Connection, actor: &Actor, appointment_id: &str) -> Result<()> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM recall WHERE appointment_id = ?1 AND status = 'booked' AND deleted_at IS NULL",
        )?;
        let v = stmt.query_map([appointment_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        v
    };
    for id in ids {
        let before = get_recall(conn, &id)?;
        conn.execute(
            "UPDATE recall SET status='pending', appointment_id=NULL, updated_at=?1, updated_by=?2, version=version+1 WHERE id=?3",
            params![now_iso(), actor.user_id, id],
        )?;
        let after = get_recall(conn, &id)?;
        audit::record(
            conn,
            actor,
            "recall.reopened",
            Some("recall"),
            Some(&id),
            Some(&json!(before)),
            Some(&json!(after)),
        )?;
    }
    Ok(())
}

/// The booked visit was moved to a new appointment record (reschedule).
pub(crate) fn move_booking(conn: &Connection, from_appointment: &str, to_appointment: &str) -> Result<()> {
    conn.execute(
        "UPDATE recall SET appointment_id = ?2 WHERE appointment_id = ?1 AND status = 'booked'",
        params![from_appointment, to_appointment],
    )?;
    Ok(())
}

/// The visit was completed: its recall is `done`, and a recurring one schedules its successor.
pub(crate) fn complete_for_appointment(
    conn: &Connection,
    actor: &Actor,
    appointment_id: &str,
    patient_id: &str,
    completed_on: Date,
) -> Result<()> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM recall WHERE appointment_id = ?1 AND status = 'booked' AND deleted_at IS NULL",
        )?;
        let v = stmt.query_map([appointment_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        v
    };
    for id in ids {
        let before = get_recall(conn, &id)?;
        conn.execute(
            "UPDATE recall SET status='done', updated_at=?1, updated_by=?2, version=version+1 WHERE id=?3",
            params![now_iso(), actor.user_id, id],
        )?;
        let after = get_recall(conn, &id)?;
        audit::record(
            conn,
            actor,
            "recall.done",
            Some("recall"),
            Some(&id),
            Some(&json!(before)),
            Some(&json!(after)),
        )?;
        if let Some(months) = before.repeat_months {
            let next = format_date(add_months(completed_on, months));
            insert(
                conn,
                actor,
                patient_id,
                before.kind,
                &next,
                Some(months),
                before.note.as_deref(),
                Some(appointment_id),
            )?;
        }
    }
    Ok(())
}

/// A patient who did not come needs a call, unless one is already waiting.
pub(crate) fn create_no_show_recall(
    conn: &Connection,
    actor: &Actor,
    patient_id: &str,
    appointment_id: &str,
    today: &str,
) -> Result<()> {
    let open: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM recall WHERE patient_id = ?1 AND kind = 'no_show'
                       AND status IN ('pending', 'contacted') AND deleted_at IS NULL)",
        [patient_id],
        |r| r.get(0),
    )?;
    if !open {
        insert(conn, actor, patient_id, RecallKind::NoShow, today, None, None, Some(appointment_id))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn add_months_clamps_to_month_length() {
        assert_eq!(add_months(date!(2026 - 01 - 31), 1), date!(2026 - 02 - 28));
        assert_eq!(add_months(date!(2028 - 01 - 31), 1), date!(2028 - 02 - 29));
        assert_eq!(add_months(date!(2026 - 10 - 03), 6), date!(2027 - 04 - 03));
        assert_eq!(add_months(date!(2026 - 11 - 15), 3), date!(2027 - 02 - 15));
        assert_eq!(add_months(date!(2026 - 12 - 31), 12), date!(2027 - 12 - 31));
    }
}
