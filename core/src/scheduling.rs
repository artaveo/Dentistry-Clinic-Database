//! Doctors, chairs and doctor schedules (roadmap 4.2): the profile a doctor
//! has in the calendar (specialty, colour, status), the chairs/rooms of the
//! clinic, and each doctor's weekly working hours, breaks, allowed chairs and
//! leave days. The appointment engine (`appointment.rs`) asks
//! [`schedule_issue`] whether a slot respects all of this.
//!
//! Days of the week are 0 = Saturday … 6 = Friday (ADR-21); times of day in a
//! schedule are stored as minutes after local midnight and exchanged as 24-hour
//! "HH:MM" text (the UI shows them 12-hour, OF-007).

use std::collections::HashMap;

use artaveo_shared::{
    ActiveStatus, AddLeaveParams, ChairInfo, CreateChairParams, CreateDoctorParams, DoctorInfo, ErrorCode,
    LeaveInfo, LeaveResult, ScheduleSlot, SetScheduleParams, UpdateChairParams, UpdateDoctorParams,
    ValidationRule,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use time::macros::format_description;
use time::Date;

use crate::audit::{self, Actor};
use crate::clinic::validate_color;
use crate::clock::now_iso;
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::new_id;

// ───────────────────────────── shared helpers ─────────────────────────────

const ISO_DATE: &[time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day]");

/// Statuses in which an appointment occupies its doctor, chair and patient.
pub(crate) const LIVE_STATUSES_SQL: &str = "('scheduled', 'confirmed', 'checked_in', 'in_treatment')";

pub(crate) fn parse_date(field: &'static str, s: &str) -> Result<Date> {
    Date::parse(s, &ISO_DATE).map_err(|_| {
        CoreError::invalid(field, ValidationRule::DateFormat, format!("`{s}` is not a YYYY-MM-DD date"))
    })
}

pub(crate) fn format_date(d: Date) -> String {
    d.format(&ISO_DATE).expect("formatting a valid date cannot fail")
}

/// Saturday = 0 … Friday = 6.
pub(crate) fn weekday_sat0(d: Date) -> u8 {
    (d.weekday().number_days_from_sunday() + 1) % 7
}

/// "H:MM" / "HH:MM" → minutes after midnight. `24:00` is accepted as an end time only.
pub(crate) fn parse_minutes(field: &'static str, s: &str, allow_24: bool) -> Result<i64> {
    let bad = || CoreError::invalid(field, ValidationRule::TimeFormat, format!("`{s}` is not an HH:MM time"));
    let (h, m) = s.trim().split_once(':').ok_or_else(bad)?;
    if h.is_empty() || h.len() > 2 || m.len() != 2 {
        return Err(bad());
    }
    let h: i64 = h.parse().map_err(|_| bad())?;
    let m: i64 = m.parse().map_err(|_| bad())?;
    let ok = m <= 59 && (h <= 23 || (allow_24 && h == 24 && m == 0));
    if !ok {
        return Err(bad());
    }
    Ok(h * 60 + m)
}

pub(crate) fn format_minutes(min: i64) -> String {
    format!("{:02}:{:02}", min / 60, min % 60)
}

// ───────────────────────────── doctors ─────────────────────────────

fn status_from(s: &str) -> ActiveStatus {
    if s == "inactive" {
        ActiveStatus::Inactive
    } else {
        ActiveStatus::Active
    }
}

fn status_code(s: ActiveStatus) -> &'static str {
    match s {
        ActiveStatus::Active => "active",
        ActiveStatus::Inactive => "inactive",
    }
}

const DOCTOR_SELECT: &str = "SELECT d.id, d.user_id, u.username, d.full_name, d.specialty, d.color, d.status,
        d.sort_order, d.version
     FROM doctor d LEFT JOIN app_user u ON u.id = d.user_id AND u.deleted_at IS NULL
     WHERE d.deleted_at IS NULL";

struct DoctorRow {
    id: String,
    user_id: Option<String>,
    username: Option<String>,
    full_name: String,
    specialty: Option<String>,
    color: String,
    status: String,
    sort_order: i64,
    version: i64,
}

fn map_doctor_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DoctorRow> {
    Ok(DoctorRow {
        id: r.get(0)?,
        user_id: r.get(1)?,
        username: r.get(2)?,
        full_name: r.get(3)?,
        specialty: r.get(4)?,
        color: r.get(5)?,
        status: r.get(6)?,
        sort_order: r.get(7)?,
        version: r.get(8)?,
    })
}

fn slots(conn: &Connection, table: &str) -> Result<HashMap<String, Vec<ScheduleSlot>>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT doctor_id, day, start_min, end_min FROM {table} ORDER BY day, start_min"
    ))?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
    })?;
    let mut map: HashMap<String, Vec<ScheduleSlot>> = HashMap::new();
    for row in rows {
        let (doctor_id, day, start, end) = row?;
        map.entry(doctor_id).or_default().push(ScheduleSlot {
            day: day as u8,
            start: format_minutes(start),
            end: format_minutes(end),
        });
    }
    Ok(map)
}

fn map_leave(r: &rusqlite::Row<'_>) -> rusqlite::Result<LeaveInfo> {
    Ok(LeaveInfo {
        id: r.get(0)?,
        doctor_id: r.get(1)?,
        start_date: r.get(2)?,
        end_date: r.get(3)?,
        reason: r.get(4)?,
        version: r.get(5)?,
    })
}

fn assemble(conn: &Connection, rows: Vec<DoctorRow>) -> Result<Vec<DoctorInfo>> {
    let mut hours = slots(conn, "doctor_hours")?;
    let mut breaks = slots(conn, "doctor_break")?;
    let mut chairs: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT doctor_id, chair_id FROM doctor_chair ORDER BY chair_id")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (d, c) = row?;
            chairs.entry(d).or_default().push(c);
        }
    }
    let mut leaves: HashMap<String, Vec<LeaveInfo>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, doctor_id, start_date, end_date, reason, version FROM doctor_leave
             WHERE deleted_at IS NULL ORDER BY start_date",
        )?;
        for l in stmt.query_map([], map_leave)? {
            let l = l?;
            leaves.entry(l.doctor_id.clone()).or_default().push(l);
        }
    }
    Ok(rows
        .into_iter()
        .map(|d| DoctorInfo {
            hours: hours.remove(&d.id).unwrap_or_default(),
            breaks: breaks.remove(&d.id).unwrap_or_default(),
            chair_ids: chairs.remove(&d.id).unwrap_or_default(),
            leaves: leaves.remove(&d.id).unwrap_or_default(),
            status: status_from(&d.status),
            id: d.id,
            user_id: d.user_id,
            username: d.username,
            full_name: d.full_name,
            specialty: d.specialty,
            color: d.color,
            sort_order: d.sort_order,
            version: d.version,
        })
        .collect())
}

pub fn list_doctors(conn: &Connection, include_inactive: bool) -> Result<Vec<DoctorInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{DOCTOR_SELECT} AND (?1 = 1 OR d.status = 'active') ORDER BY d.sort_order, d.full_name COLLATE NOCASE"
    ))?;
    let rows: Vec<DoctorRow> =
        stmt.query_map([include_inactive as i64], map_doctor_row)?.collect::<rusqlite::Result<_>>()?;
    assemble(conn, rows)
}

pub fn get_doctor(conn: &Connection, id: &str) -> Result<DoctorInfo> {
    let row = conn
        .query_row(&format!("{DOCTOR_SELECT} AND d.id = ?1"), [id], map_doctor_row)
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("doctor {id}"))
                .on_field("doctor_id", ValidationRule::DoctorNotFound)
        })?;
    Ok(assemble(conn, vec![row])?.remove(0))
}

fn validate_doctor_fields(full_name: &str, specialty: Option<&str>, color: &str) -> Result<()> {
    let len = full_name.trim().chars().count();
    if !(1..=100).contains(&len) {
        return Err(CoreError::invalid(
            "full_name",
            ValidationRule::FullNameLength,
            "doctor name must be 1-100 characters",
        ));
    }
    if specialty.is_some_and(|s| s.trim().chars().count() > 100) {
        return Err(CoreError::invalid(
            "specialty",
            ValidationRule::SpecialtyLength,
            "specialty must be at most 100 characters",
        ));
    }
    validate_color("color", color)
}

/// A login may be linked to one doctor only, and must be an existing active user.
fn check_user_link(conn: &Connection, user_id: Option<&str>, except_doctor: Option<&str>) -> Result<()> {
    let Some(uid) = user_id else { return Ok(()) };
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM app_user WHERE id = ?1 AND deleted_at IS NULL)",
        [uid],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(CoreError::invalid(
            "user_id",
            ValidationRule::UserNotFound,
            format!("unknown user {uid}"),
        ));
    }
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM doctor WHERE user_id = ?1 AND deleted_at IS NULL AND id IS NOT ?2)",
        params![uid, except_doctor],
        |r| r.get(0),
    )?;
    if taken {
        return Err(CoreError::invalid(
            "user_id",
            ValidationRule::UserAlreadyDoctor,
            "this user is already linked to another doctor",
        ));
    }
    Ok(())
}

pub fn create_doctor(conn: &Connection, actor: &Actor, p: &CreateDoctorParams) -> Result<DoctorInfo> {
    validate_doctor_fields(&p.full_name, p.specialty.as_deref(), &p.color)?;
    let user_id = p.user_id.as_deref().filter(|s| !s.is_empty());
    check_user_link(conn, user_id, None)?;
    let id = new_id();
    let now = now_iso();
    let order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM doctor WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO doctor(id, user_id, full_name, specialty, color, sort_order, created_at, created_by, updated_at, updated_by)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?7,?8)",
        params![
            id,
            user_id,
            p.full_name.trim(),
            p.specialty.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            p.color.to_ascii_lowercase(),
            order,
            now,
            actor.user_id
        ],
    )?;
    let info = get_doctor(conn, &id)?;
    audit::record(conn, actor, "doctor.create", Some("doctor"), Some(&id), None, Some(&json!(info)))?;
    Ok(info)
}

pub fn update_doctor(conn: &Connection, actor: &Actor, p: &UpdateDoctorParams) -> Result<DoctorInfo> {
    validate_doctor_fields(&p.full_name, p.specialty.as_deref(), &p.color)?;
    let before = get_doctor(conn, &p.id)?;
    let user_id = p.user_id.as_deref().filter(|s| !s.is_empty());
    check_user_link(conn, user_id, Some(&p.id))?;
    let changed = conn.execute(
        "UPDATE doctor SET user_id=?1, full_name=?2, specialty=?3, color=?4, status=?5,
            updated_at=?6, updated_by=?7, version=version+1
         WHERE id=?8 AND version=?9 AND deleted_at IS NULL",
        params![
            user_id,
            p.full_name.trim(),
            p.specialty.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            p.color.to_ascii_lowercase(),
            status_code(p.status),
            now_iso(),
            actor.user_id,
            p.id,
            p.version
        ],
    )?;
    expect_one_row(changed, "doctor")?;
    let after = get_doctor(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        "doctor.update",
        Some("doctor"),
        Some(&p.id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

fn validate_slots(field: &'static str, list: &[ScheduleSlot]) -> Result<Vec<(i64, i64, i64)>> {
    let mut parsed = Vec::with_capacity(list.len());
    for s in list {
        if s.day > 6 {
            return Err(CoreError::invalid(
                field,
                ValidationRule::WorkingHours,
                "day must be 0 (Saturday) to 6 (Friday)",
            ));
        }
        let start = parse_minutes(field, &s.start, false)?;
        let end = parse_minutes(field, &s.end, true)?;
        if end <= start {
            return Err(CoreError::invalid(field, ValidationRule::TimeRange, "end must be after start"));
        }
        parsed.push((s.day as i64, start, end));
    }
    parsed.sort_unstable();
    for w in parsed.windows(2) {
        if w[0].0 == w[1].0 && w[1].1 < w[0].2 {
            return Err(CoreError::invalid(field, ValidationRule::ScheduleOverlap, "intervals overlap"));
        }
    }
    Ok(parsed)
}

/// Replaces a doctor's whole weekly schedule: working hours, breaks and allowed chairs.
pub fn set_schedule(conn: &Connection, actor: &Actor, p: &SetScheduleParams) -> Result<DoctorInfo> {
    let before = get_doctor(conn, &p.doctor_id)?;
    let hours = validate_slots("hours", &p.hours)?;
    let breaks = validate_slots("breaks", &p.breaks)?;
    let mut chair_ids = p.chair_ids.clone();
    chair_ids.sort();
    chair_ids.dedup();
    for c in &chair_ids {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM chair WHERE id = ?1 AND deleted_at IS NULL)",
            [c],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(CoreError::invalid(
                "chair_ids",
                ValidationRule::ChairNotFound,
                format!("unknown chair {c}"),
            ));
        }
    }
    let changed = conn.execute(
        "UPDATE doctor SET updated_at=?1, updated_by=?2, version=version+1
         WHERE id=?3 AND version=?4 AND deleted_at IS NULL",
        params![now_iso(), actor.user_id, p.doctor_id, p.version],
    )?;
    expect_one_row(changed, "doctor")?;
    for table in ["doctor_hours", "doctor_break"] {
        conn.execute(&format!("DELETE FROM {table} WHERE doctor_id = ?1"), [&p.doctor_id])?;
    }
    conn.execute("DELETE FROM doctor_chair WHERE doctor_id = ?1", [&p.doctor_id])?;
    for (table, list) in [("doctor_hours", &hours), ("doctor_break", &breaks)] {
        for (day, start, end) in list {
            conn.execute(
                &format!(
                    "INSERT INTO {table}(id, doctor_id, day, start_min, end_min) VALUES (?1,?2,?3,?4,?5)"
                ),
                params![new_id(), p.doctor_id, day, start, end],
            )?;
        }
    }
    for c in &chair_ids {
        conn.execute(
            "INSERT INTO doctor_chair(doctor_id, chair_id) VALUES (?1, ?2)",
            params![p.doctor_id, c],
        )?;
    }
    let after = get_doctor(conn, &p.doctor_id)?;
    audit::record(
        conn,
        actor,
        "doctor.set_schedule",
        Some("doctor"),
        Some(&p.doctor_id),
        Some(&json!({ "hours": before.hours, "breaks": before.breaks, "chair_ids": before.chair_ids })),
        Some(&json!({ "hours": after.hours, "breaks": after.breaks, "chair_ids": after.chair_ids })),
    )?;
    Ok(after)
}

pub fn add_leave(conn: &Connection, actor: &Actor, p: &AddLeaveParams) -> Result<LeaveResult> {
    get_doctor(conn, &p.doctor_id)?;
    let start = parse_date("start_date", &p.start_date)?;
    let end = parse_date("end_date", &p.end_date)?;
    if end < start {
        return Err(CoreError::invalid(
            "end_date",
            ValidationRule::LeaveRange,
            "the last day is before the first",
        ));
    }
    let reason = p.reason.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if reason.is_some_and(|r| r.chars().count() > 200) {
        return Err(CoreError::invalid(
            "reason",
            ValidationRule::Required,
            "reason must be at most 200 characters",
        ));
    }
    let id = new_id();
    let now = now_iso();
    conn.execute(
        "INSERT INTO doctor_leave(id, doctor_id, start_date, end_date, reason, created_at, created_by, updated_at, updated_by)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?6,?7)",
        params![id, p.doctor_id, format_date(start), format_date(end), reason, now, actor.user_id],
    )?;
    let leave = conn.query_row(
        "SELECT id, doctor_id, start_date, end_date, reason, version FROM doctor_leave WHERE id = ?1",
        [&id],
        map_leave,
    )?;
    let affected: u32 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM appointment WHERE doctor_id = ?1 AND deleted_at IS NULL
             AND status IN {LIVE_STATUSES_SQL} AND local_date BETWEEN ?2 AND ?3"
        ),
        params![p.doctor_id, leave.start_date, leave.end_date],
        |r| r.get(0),
    )?;
    audit::record(
        conn,
        actor,
        "doctor.leave_add",
        Some("doctor"),
        Some(&p.doctor_id),
        None,
        Some(&json!({ "leave": leave, "affected_appointments": affected })),
    )?;
    Ok(LeaveResult { leave, affected_appointments: affected })
}

pub fn delete_leave(conn: &Connection, actor: &Actor, id: &str, version: i64) -> Result<()> {
    let before = conn
        .query_row(
            "SELECT id, doctor_id, start_date, end_date, reason, version FROM doctor_leave
             WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_leave,
        )
        .optional()?
        .ok_or_else(|| CoreError::api(ErrorCode::NotFound, format!("leave {id}")))?;
    let changed = conn.execute(
        "UPDATE doctor_leave SET deleted_at=?1, updated_at=?1, updated_by=?2, version=version+1
         WHERE id=?3 AND version=?4 AND deleted_at IS NULL",
        params![now_iso(), actor.user_id, id, version],
    )?;
    expect_one_row(changed, "leave")?;
    audit::record(
        conn,
        actor,
        "doctor.leave_delete",
        Some("doctor"),
        Some(&before.doctor_id),
        Some(&json!(before)),
        None,
    )?;
    Ok(())
}

/// Why a slot does not fit a doctor's schedule (4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduleIssue {
    OnLeave,
    OutsideHours,
    OnBreak,
}

impl ScheduleIssue {
    pub(crate) fn into_error(self) -> CoreError {
        match self {
            ScheduleIssue::OnLeave => CoreError::invalid(
                "date",
                ValidationRule::DoctorOnLeave,
                "the doctor is on leave on this day",
            ),
            ScheduleIssue::OutsideHours => CoreError::invalid(
                "start_time",
                ValidationRule::OutsideWorkingHours,
                "the slot is outside the doctor's working hours",
            ),
            ScheduleIssue::OnBreak => CoreError::invalid(
                "start_time",
                ValidationRule::DoctorOnBreak,
                "the slot overlaps the doctor's break",
            ),
        }
    }
}

/// Checks a slot against the doctor's leave, weekly hours and breaks. A doctor
/// with no weekly hours at all has no hour restriction (a freshly added doctor
/// can be booked before anyone has typed in a schedule); leave always applies.
pub(crate) fn schedule_issue(
    conn: &Connection,
    doctor_id: &str,
    date: Date,
    start_min: i64,
    end_min: i64,
) -> Result<Option<ScheduleIssue>> {
    let iso = format_date(date);
    let on_leave: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM doctor_leave WHERE doctor_id = ?1 AND deleted_at IS NULL
                       AND ?2 BETWEEN start_date AND end_date)",
        params![doctor_id, iso],
        |r| r.get(0),
    )?;
    if on_leave {
        return Ok(Some(ScheduleIssue::OnLeave));
    }
    let any_hours: bool =
        conn.query_row("SELECT EXISTS(SELECT 1 FROM doctor_hours WHERE doctor_id = ?1)", [doctor_id], |r| {
            r.get(0)
        })?;
    let day = weekday_sat0(date) as i64;
    if any_hours {
        let inside: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM doctor_hours WHERE doctor_id = ?1 AND day = ?2
                           AND start_min <= ?3 AND end_min >= ?4)",
            params![doctor_id, day, start_min, end_min],
            |r| r.get(0),
        )?;
        if !inside {
            return Ok(Some(ScheduleIssue::OutsideHours));
        }
    }
    let on_break: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM doctor_break WHERE doctor_id = ?1 AND day = ?2
                       AND start_min < ?4 AND end_min > ?3)",
        params![doctor_id, day, start_min, end_min],
        |r| r.get(0),
    )?;
    Ok(on_break.then_some(ScheduleIssue::OnBreak))
}

// ───────────────────────────── chairs ─────────────────────────────

const CHAIR_SELECT: &str = "SELECT id, name, status, sort_order, version FROM chair WHERE deleted_at IS NULL";

fn map_chair(r: &rusqlite::Row<'_>) -> rusqlite::Result<ChairInfo> {
    Ok(ChairInfo {
        id: r.get(0)?,
        name: r.get(1)?,
        status: status_from(&r.get::<_, String>(2)?),
        sort_order: r.get(3)?,
        version: r.get(4)?,
    })
}

pub fn list_chairs(conn: &Connection, include_inactive: bool) -> Result<Vec<ChairInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{CHAIR_SELECT} AND (?1 = 1 OR status = 'active') ORDER BY sort_order, name COLLATE NOCASE"
    ))?;
    let v = stmt.query_map([include_inactive as i64], map_chair)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn get_chair(conn: &Connection, id: &str) -> Result<ChairInfo> {
    conn.query_row(&format!("{CHAIR_SELECT} AND id = ?1"), [id], map_chair).optional()?.ok_or_else(|| {
        CoreError::api(ErrorCode::NotFound, format!("chair {id}"))
            .on_field("chair_id", ValidationRule::ChairNotFound)
    })
}

fn validate_chair_name(conn: &Connection, name: &str, except: Option<&str>) -> Result<()> {
    let len = name.trim().chars().count();
    if !(1..=40).contains(&len) {
        return Err(CoreError::invalid(
            "name",
            ValidationRule::ChairNameLength,
            "chair name must be 1-40 characters",
        ));
    }
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM chair WHERE name = ?1 COLLATE NOCASE AND deleted_at IS NULL AND id IS NOT ?2)",
        params![name.trim(), except],
        |r| r.get(0),
    )?;
    if taken {
        return Err(CoreError::invalid(
            "name",
            ValidationRule::ChairNameTaken,
            "a chair with this name exists",
        ));
    }
    Ok(())
}

pub fn create_chair(conn: &Connection, actor: &Actor, p: &CreateChairParams) -> Result<ChairInfo> {
    validate_chair_name(conn, &p.name, None)?;
    let id = new_id();
    let now = now_iso();
    let order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM chair WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO chair(id, name, sort_order, created_at, created_by, updated_at, updated_by)
         VALUES (?1,?2,?3,?4,?5,?4,?5)",
        params![id, p.name.trim(), order, now, actor.user_id],
    )?;
    let info = get_chair(conn, &id)?;
    audit::record(conn, actor, "chair.create", Some("chair"), Some(&id), None, Some(&json!(info)))?;
    Ok(info)
}

pub fn update_chair(conn: &Connection, actor: &Actor, p: &UpdateChairParams) -> Result<ChairInfo> {
    let before = get_chair(conn, &p.id)?;
    validate_chair_name(conn, &p.name, Some(&p.id))?;
    let changed = conn.execute(
        "UPDATE chair SET name=?1, status=?2, updated_at=?3, updated_by=?4, version=version+1
         WHERE id=?5 AND version=?6 AND deleted_at IS NULL",
        params![p.name.trim(), status_code(p.status), now_iso(), actor.user_id, p.id, p.version],
    )?;
    expect_one_row(changed, "chair")?;
    let after = get_chair(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        "chair.update",
        Some("chair"),
        Some(&p.id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn minutes_parse_and_format() {
        assert_eq!(parse_minutes("t", "08:30", false).unwrap(), 510);
        assert_eq!(parse_minutes("t", "8:05", false).unwrap(), 485);
        assert_eq!(parse_minutes("t", "24:00", true).unwrap(), 1440);
        assert!(parse_minutes("t", "24:00", false).is_err());
        assert!(parse_minutes("t", "12:60", false).is_err());
        assert!(parse_minutes("t", "noon", false).is_err());
        assert!(parse_minutes("t", "9:5", false).is_err());
        assert_eq!(format_minutes(510), "08:30");
        assert_eq!(format_minutes(1440), "24:00");
    }

    #[test]
    fn afghan_week_starts_on_saturday() {
        assert_eq!(weekday_sat0(date!(2026 - 10 - 03)), 0, "2026-10-03 is a Saturday");
        assert_eq!(weekday_sat0(date!(2026 - 10 - 04)), 1);
        assert_eq!(weekday_sat0(date!(2026 - 10 - 09)), 6, "Friday");
    }

    #[test]
    fn overlapping_intervals_are_rejected() {
        let slot = |d: u8, a: &str, b: &str| ScheduleSlot { day: d, start: a.into(), end: b.into() };
        assert!(validate_slots("hours", &[slot(0, "08:00", "12:00"), slot(0, "14:00", "18:00")]).is_ok());
        assert!(validate_slots("hours", &[slot(0, "08:00", "12:00"), slot(1, "08:00", "12:00")]).is_ok());
        assert!(validate_slots("hours", &[slot(0, "08:00", "12:00"), slot(0, "11:00", "13:00")]).is_err());
        assert!(validate_slots("hours", &[slot(0, "12:00", "08:00")]).is_err());
        assert!(validate_slots("hours", &[slot(7, "08:00", "12:00")]).is_err());
    }
}
