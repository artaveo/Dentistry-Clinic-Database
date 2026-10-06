//! Appointment engine and reception workflow (roadmap 4.3–4.5).
//!
//! * An appointment is a patient + doctor (+ optional chair) in a time slot of
//!   one clinic-local day. Instants are stored in UTC (ADR-07) next to the
//!   local date and minutes, so day queries are index lookups.
//! * **No double booking:** a doctor, a chair and a patient can each be in
//!   only one *live* appointment (scheduled, confirmed, checked-in, in
//!   treatment) at a time. Completed, cancelled, no-show and rescheduled
//!   visits free their slot.
//! * Slots also respect the doctor's schedule (hours, breaks, leave) unless the
//!   caller explicitly overrides it.
//! * Status workflow (4.4) and the reception queue (4.5):
//!
//! ```text
//! Scheduled → Confirmed → Checked-in → In Treatment → Completed
//!                     ↘ Cancelled · No-show · Rescheduled
//! ```

use artaveo_shared::{
    AppointmentCountsParams, AppointmentInfo, AppointmentListParams, AppointmentStatus,
    CreateAppointmentParams, DayCount, ErrorCode, RescheduleAppointmentParams, SetAppointmentStatusParams,
    UpdateAppointmentParams, ValidationRule, WalkInParams,
};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::json;
use time::{Date, PrimitiveDateTime, Time};

use crate::audit::{self, Actor};
use crate::clock::{iso, local_now, now_iso, today_iso, CLINIC_OFFSET};
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::new_id;
use crate::recall;
use crate::scheduling::{
    format_date, format_minutes, parse_date, parse_minutes, schedule_issue, LIVE_STATUSES_SQL,
};

const MIN_MINUTES: i64 = 5;
const MAX_MINUTES: i64 = 12 * 60;
const DEFAULT_WALK_IN_MINUTES: i64 = 30;

// ───────────────────────────── reads ─────────────────────────────

const APPOINTMENT_SELECT: &str = "SELECT a.id, a.patient_id, p.patient_number, p.full_name, p.phone,
        a.doctor_id, d.full_name, d.color, a.chair_id, c.name,
        a.local_date, a.start_min, a.end_min, a.start_at, a.end_at, a.reason, a.notes, a.status,
        a.is_walk_in, a.queue_number, a.checked_in_at, a.treatment_started_at, a.completed_at,
        a.cancelled_at, a.cancel_reason, a.rescheduled_from_id, a.rescheduled_to_id, a.version
     FROM appointment a
     JOIN patient p ON p.id = a.patient_id
     JOIN doctor d ON d.id = a.doctor_id
     LEFT JOIN chair c ON c.id = a.chair_id
     WHERE a.deleted_at IS NULL";

fn map_appointment(r: &rusqlite::Row<'_>) -> rusqlite::Result<AppointmentInfo> {
    Ok(AppointmentInfo {
        id: r.get(0)?,
        patient_id: r.get(1)?,
        patient_number: r.get(2)?,
        patient_name: r.get(3)?,
        patient_phone: r.get(4)?,
        doctor_id: r.get(5)?,
        doctor_name: r.get(6)?,
        doctor_color: r.get(7)?,
        chair_id: r.get(8)?,
        chair_name: r.get(9)?,
        date: r.get(10)?,
        start_time: format_minutes(r.get(11)?),
        end_time: format_minutes(r.get(12)?),
        start_at: r.get(13)?,
        end_at: r.get(14)?,
        reason: r.get(15)?,
        notes: r.get(16)?,
        status: AppointmentStatus::from_code(&r.get::<_, String>(17)?)
            .unwrap_or(AppointmentStatus::Scheduled),
        is_walk_in: r.get::<_, i64>(18)? == 1,
        queue_number: r.get(19)?,
        checked_in_at: r.get(20)?,
        treatment_started_at: r.get(21)?,
        completed_at: r.get(22)?,
        cancelled_at: r.get(23)?,
        cancel_reason: r.get(24)?,
        rescheduled_from_id: r.get(25)?,
        rescheduled_to_id: r.get(26)?,
        version: r.get(27)?,
    })
}

pub fn get_appointment(conn: &Connection, id: &str) -> Result<AppointmentInfo> {
    conn.query_row(&format!("{APPOINTMENT_SELECT} AND a.id = ?1"), [id], map_appointment)
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("appointment {id}"))
                .on_field("id", ValidationRule::AppointmentNotFound)
        })
}

/// Appointments of a date range and/or of one patient. Day views order by time;
/// a patient's history lists the newest first.
pub fn list_appointments(conn: &Connection, p: &AppointmentListParams) -> Result<Vec<AppointmentInfo>> {
    let from = p.date_from.as_deref().filter(|s| !s.is_empty());
    let to = p.date_to.as_deref().filter(|s| !s.is_empty());
    if from.is_none() && to.is_none() && p.patient_id.is_none() {
        return Err(CoreError::invalid(
            "date_from",
            ValidationRule::Required,
            "a date range or a patient is required",
        ));
    }
    let mut sql = String::new();
    let mut args: Vec<Value> = Vec::new();
    if let Some(d) = from {
        sql.push_str(" AND a.local_date >= ?");
        args.push(format_date(parse_date("date_from", d)?).into());
    }
    if let Some(d) = to {
        sql.push_str(" AND a.local_date <= ?");
        args.push(format_date(parse_date("date_to", d)?).into());
    }
    for (col, v) in
        [("a.doctor_id", &p.doctor_id), ("a.chair_id", &p.chair_id), ("a.patient_id", &p.patient_id)]
    {
        if let Some(v) = v.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(&format!(" AND {col} = ?"));
            args.push(v.to_string().into());
        }
    }
    if !p.statuses.is_empty() {
        sql.push_str(&format!(" AND a.status IN ({})", vec!["?"; p.statuses.len()].join(",")));
        args.extend(p.statuses.iter().map(|s| Value::from(s.code().to_string())));
    }
    let order = if p.patient_id.is_some() && from.is_none() && to.is_none() {
        "a.start_at DESC"
    } else {
        "a.local_date, a.start_min, a.created_at"
    };
    let limit = p.limit.clamp(1, 2000);
    let mut stmt = conn
        .prepare(&format!("{APPOINTMENT_SELECT}{sql} ORDER BY {order} LIMIT {limit} OFFSET {}", p.offset))?;
    let rows =
        stmt.query_map(params_from_iter(args.iter()), map_appointment)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Per-day totals for the month view of the calendar.
pub fn appointment_counts(conn: &Connection, p: &AppointmentCountsParams) -> Result<Vec<DayCount>> {
    let from = parse_date("date_from", &p.date_from)?;
    let to = parse_date("date_to", &p.date_to)?;
    if to < from || (to - from).whole_days() > 400 {
        return Err(CoreError::invalid(
            "date_to",
            ValidationRule::TimeRange,
            "date range must be 0-400 days",
        ));
    }
    let mut stmt = conn.prepare(&format!(
        "SELECT local_date, COUNT(*), SUM(CASE WHEN status IN {LIVE_STATUSES_SQL} THEN 1 ELSE 0 END)
         FROM appointment
         WHERE deleted_at IS NULL AND status NOT IN ('cancelled', 'rescheduled')
           AND local_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR doctor_id = ?3)
         GROUP BY local_date ORDER BY local_date"
    ))?;
    let v = stmt
        .query_map(
            params![format_date(from), format_date(to), p.doctor_id.as_deref().filter(|s| !s.is_empty())],
            |r| Ok(DayCount { date: r.get(0)?, total: r.get(1)?, open: r.get(2)? }),
        )?
        .collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

// ───────────────────────────── slots ─────────────────────────────

/// A validated slot: one clinic-local day, minutes after midnight.
#[derive(Clone, Copy)]
struct Slot {
    date: Date,
    start_min: i64,
    end_min: i64,
}

impl Slot {
    fn parse(date: &str, start: &str, end: &str) -> Result<Slot> {
        let date = parse_date("date", date)?;
        let start_min = parse_minutes("start_time", start, false)?;
        let end_min = parse_minutes("end_time", end, true)?;
        if end_min <= start_min {
            return Err(CoreError::invalid(
                "end_time",
                ValidationRule::TimeRange,
                "the end must be after the start",
            ));
        }
        let len = end_min - start_min;
        if !(MIN_MINUTES..=MAX_MINUTES).contains(&len) {
            return Err(CoreError::invalid(
                "end_time",
                ValidationRule::DurationRange,
                "an appointment lasts 5 minutes to 12 hours",
            ));
        }
        Ok(Slot { date, start_min, end_min })
    }

    /// UTC instant of `min` minutes after local midnight of the slot's day.
    fn instant(&self, min: i64) -> String {
        iso(PrimitiveDateTime::new(self.date, Time::MIDNIGHT).assume_offset(CLINIC_OFFSET)
            + time::Duration::minutes(min))
    }

    fn start_at(&self) -> String {
        self.instant(self.start_min)
    }

    fn end_at(&self) -> String {
        self.instant(self.end_min)
    }
}

fn clean(s: &Option<String>) -> Option<String> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn validate_texts(reason: &Option<String>, notes: &Option<String>) -> Result<()> {
    if reason.as_deref().is_some_and(|s| s.trim().chars().count() > 200) {
        return Err(CoreError::invalid(
            "reason",
            ValidationRule::Required,
            "reason must be at most 200 characters",
        ));
    }
    if notes.as_deref().is_some_and(|s| s.trim().chars().count() > 1000) {
        return Err(CoreError::invalid(
            "notes",
            ValidationRule::Required,
            "notes must be at most 1000 characters",
        ));
    }
    Ok(())
}

// ───────────────────────────── entity & conflict checks ─────────────────────────────

fn check_patient(conn: &Connection, patient_id: &str) -> Result<()> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT merged_into_id FROM patient WHERE id = ?1 AND deleted_at IS NULL",
            [patient_id],
            |r| r.get(0),
        )
        .optional()?;
    match row {
        None => Err(CoreError::invalid("patient_id", ValidationRule::PatientNotFound, "unknown patient")),
        Some(Some(_)) => Err(CoreError::invalid(
            "patient_id",
            ValidationRule::PatientMerged,
            "this record was merged into another patient",
        )),
        Some(None) => Ok(()),
    }
}

fn check_doctor(conn: &Connection, doctor_id: &str, must_be_active: bool) -> Result<()> {
    let status: Option<String> = conn
        .query_row("SELECT status FROM doctor WHERE id = ?1 AND deleted_at IS NULL", [doctor_id], |r| {
            r.get(0)
        })
        .optional()?;
    match status.as_deref() {
        None => Err(CoreError::invalid("doctor_id", ValidationRule::DoctorNotFound, "unknown doctor")),
        Some("inactive") if must_be_active => {
            Err(CoreError::invalid("doctor_id", ValidationRule::DoctorInactive, "this doctor is inactive"))
        }
        Some(_) => Ok(()),
    }
}

fn check_chair(
    conn: &Connection,
    doctor_id: &str,
    chair_id: Option<&str>,
    must_be_active: bool,
) -> Result<()> {
    let Some(chair_id) = chair_id else { return Ok(()) };
    let status: Option<String> = conn
        .query_row("SELECT status FROM chair WHERE id = ?1 AND deleted_at IS NULL", [chair_id], |r| r.get(0))
        .optional()?;
    match status.as_deref() {
        None => return Err(CoreError::invalid("chair_id", ValidationRule::ChairNotFound, "unknown chair")),
        Some("inactive") if must_be_active => {
            return Err(CoreError::invalid(
                "chair_id",
                ValidationRule::ChairInactive,
                "this chair is inactive",
            ))
        }
        Some(_) => {}
    }
    let restricted: bool =
        conn.query_row("SELECT EXISTS(SELECT 1 FROM doctor_chair WHERE doctor_id = ?1)", [doctor_id], |r| {
            r.get(0)
        })?;
    if restricted {
        let allowed: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM doctor_chair WHERE doctor_id = ?1 AND chair_id = ?2)",
            params![doctor_id, chair_id],
            |r| r.get(0),
        )?;
        if !allowed {
            return Err(CoreError::invalid(
                "chair_id",
                ValidationRule::ChairNotAllowed,
                "this doctor does not use this chair",
            ));
        }
    }
    Ok(())
}

fn overlaps(
    conn: &Connection,
    column: &str,
    value: &str,
    slot: &Slot,
    exclude: Option<&str>,
) -> Result<bool> {
    Ok(conn.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM appointment WHERE {column} = ?1 AND deleted_at IS NULL
                           AND status IN {LIVE_STATUSES_SQL} AND id IS NOT ?2
                           AND start_at < ?4 AND end_at > ?3)"
        ),
        params![value, exclude, slot.start_at(), slot.end_at()],
        |r| r.get(0),
    )?)
}

/// OF-036: a patient holds a slot for every visit except a cancelled or moved one. A visit that
/// was finished or missed still counts, so the same patient cannot be booked again at that time.
fn patient_overlaps(conn: &Connection, patient_id: &str, slot: &Slot, exclude: Option<&str>) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM appointment WHERE patient_id = ?1 AND deleted_at IS NULL
                       AND status NOT IN ('cancelled', 'rescheduled') AND id IS NOT ?2
                       AND start_at < ?4 AND end_at > ?3)",
        params![patient_id, exclude, slot.start_at(), slot.end_at()],
        |r| r.get(0),
    )?)
}

/// OF-036: a new booking is for a time still to come. Past visits are not booked as appointments.
fn check_not_past(slot: &Slot) -> Result<()> {
    if slot.start_at() < crate::clock::booking_now_iso() {
        return Err(CoreError::invalid(
            "start_time",
            ValidationRule::AppointmentInPast,
            "the appointment is in the past",
        ));
    }
    Ok(())
}

/// OF-035: a booking without a chosen chair takes the first free chair the doctor may use, so
/// two doctors never share one dental chair by accident. A clinic without chairs keeps none.
/// When every chair is busy at that time the booking is refused.
fn resolve_chair(
    conn: &Connection,
    doctor_id: &str,
    requested: Option<&str>,
    slot: &Slot,
    exclude: Option<&str>,
) -> Result<Option<String>> {
    if let Some(chair) = requested {
        return Ok(Some(chair.to_string()));
    }
    let any_chair: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM chair WHERE deleted_at IS NULL AND status = 'active')",
        [],
        |r| r.get(0),
    )?;
    if !any_chair {
        return Ok(None);
    }
    let free: Option<String> = conn
        .query_row(
            &format!(
                "SELECT c.id FROM chair c
                 WHERE c.deleted_at IS NULL AND c.status = 'active'
                   AND (NOT EXISTS(SELECT 1 FROM doctor_chair d WHERE d.doctor_id = ?1)
                        OR EXISTS(SELECT 1 FROM doctor_chair d WHERE d.doctor_id = ?1 AND d.chair_id = c.id))
                   AND NOT EXISTS(SELECT 1 FROM appointment a WHERE a.chair_id = c.id AND a.deleted_at IS NULL
                                  AND a.status IN {LIVE_STATUSES_SQL} AND a.id IS NOT ?4
                                  AND a.start_at < ?3 AND a.end_at > ?2)
                 ORDER BY c.sort_order, c.name LIMIT 1"
            ),
            params![doctor_id, slot.start_at(), slot.end_at(), exclude],
            |r| r.get(0),
        )
        .optional()?;
    free.map(Some).ok_or_else(|| {
        CoreError::invalid("chair_id", ValidationRule::ChairBusy, "every chair is busy at this time")
    })
}

/// Double-booking prevention (4.3): doctor, chair and patient each in one place at a time.
fn check_conflicts(
    conn: &Connection,
    slot: &Slot,
    patient_id: &str,
    doctor_id: &str,
    chair_id: Option<&str>,
    exclude: Option<&str>,
) -> Result<()> {
    if overlaps(conn, "doctor_id", doctor_id, slot, exclude)? {
        return Err(CoreError::invalid(
            "doctor_id",
            ValidationRule::DoctorBusy,
            "the doctor already has an appointment at this time",
        ));
    }
    if let Some(c) = chair_id {
        if overlaps(conn, "chair_id", c, slot, exclude)? {
            return Err(CoreError::invalid(
                "chair_id",
                ValidationRule::ChairBusy,
                "the chair is already used at this time",
            ));
        }
    }
    if patient_overlaps(conn, patient_id, slot, exclude)? {
        return Err(CoreError::invalid(
            "patient_id",
            ValidationRule::PatientBusy,
            "the patient already has an appointment at this time",
        ));
    }
    Ok(())
}

fn check_schedule(conn: &Connection, doctor_id: &str, slot: &Slot, override_schedule: bool) -> Result<()> {
    if override_schedule {
        return Ok(());
    }
    match schedule_issue(conn, doctor_id, slot.date, slot.start_min, slot.end_min)? {
        Some(issue) => Err(issue.into_error()),
        None => Ok(()),
    }
}

// ───────────────────────────── writes ─────────────────────────────

struct NewAppointment<'a> {
    patient_id: &'a str,
    doctor_id: &'a str,
    chair_id: Option<&'a str>,
    slot: Slot,
    reason: Option<String>,
    notes: Option<String>,
    status: AppointmentStatus,
    is_walk_in: bool,
    rescheduled_from_id: Option<&'a str>,
}

fn next_queue_number(conn: &Connection, date: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(queue_number), 0) + 1 FROM appointment WHERE local_date = ?1 AND deleted_at IS NULL",
        [date],
        |r| r.get(0),
    )?)
}

fn insert(conn: &Connection, actor: &Actor, n: NewAppointment<'_>) -> Result<String> {
    let id = new_id();
    let now = now_iso();
    let date = format_date(n.slot.date);
    let checked_in = n.status == AppointmentStatus::CheckedIn;
    let queue_number = if checked_in { Some(next_queue_number(conn, &date)?) } else { None };
    conn.execute(
        "INSERT INTO appointment(id, patient_id, doctor_id, chair_id, start_at, end_at, local_date, start_min, end_min,
            reason, notes, status, is_walk_in, queue_number, checked_in_at, rescheduled_from_id,
            created_at, created_by, updated_at, updated_by)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?17,?18)",
        params![
            id,
            n.patient_id,
            n.doctor_id,
            n.chair_id,
            n.slot.start_at(),
            n.slot.end_at(),
            date,
            n.slot.start_min,
            n.slot.end_min,
            n.reason,
            n.notes,
            n.status.code(),
            n.is_walk_in as i64,
            queue_number,
            checked_in.then(|| now.clone()),
            n.rescheduled_from_id,
            now,
            actor.user_id
        ],
    )?;
    Ok(id)
}

pub fn create_appointment(
    conn: &Connection,
    actor: &Actor,
    p: &CreateAppointmentParams,
) -> Result<AppointmentInfo> {
    let slot = Slot::parse(&p.date, &p.start_time, &p.end_time)?;
    validate_texts(&p.reason, &p.notes)?;
    let requested = p.chair_id.as_deref().filter(|s| !s.is_empty());
    check_patient(conn, &p.patient_id)?;
    check_doctor(conn, &p.doctor_id, true)?;
    check_chair(conn, &p.doctor_id, requested, true)?;
    check_not_past(&slot)?;
    check_schedule(conn, &p.doctor_id, &slot, p.override_schedule)?;
    let chair = resolve_chair(conn, &p.doctor_id, requested, &slot, None)?;
    check_conflicts(conn, &slot, &p.patient_id, &p.doctor_id, chair.as_deref(), None)?;
    let id = insert(
        conn,
        actor,
        NewAppointment {
            patient_id: &p.patient_id,
            doctor_id: &p.doctor_id,
            chair_id: chair.as_deref(),
            slot,
            reason: clean(&p.reason),
            notes: clean(&p.notes),
            status: AppointmentStatus::Scheduled,
            is_walk_in: false,
            rescheduled_from_id: None,
        },
    )?;
    if let Some(rid) = p.recall_id.as_deref().filter(|s| !s.is_empty()) {
        recall::link_booking(conn, actor, rid, &id, &p.patient_id)?;
    }
    let info = get_appointment(conn, &id)?;
    audit::record(
        conn,
        actor,
        "appointment.create",
        Some("appointment"),
        Some(&id),
        None,
        Some(&json!(info)),
    )?;
    Ok(info)
}

/// Edits details and/or moves the appointment (drag & drop uses this call).
/// Only a scheduled or confirmed appointment can change doctor, chair or time;
/// once the patient has arrived only the reason and notes can change.
pub fn update_appointment(
    conn: &Connection,
    actor: &Actor,
    p: &UpdateAppointmentParams,
) -> Result<AppointmentInfo> {
    let slot = Slot::parse(&p.date, &p.start_time, &p.end_time)?;
    validate_texts(&p.reason, &p.notes)?;
    let before = get_appointment(conn, &p.id)?;
    let chair = p.chair_id.as_deref().filter(|s| !s.is_empty());
    use AppointmentStatus::*;
    if !matches!(before.status, Scheduled | Confirmed | CheckedIn | InTreatment) {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::NotEditable,
            "a finished appointment cannot be edited",
        ));
    }
    let moved = before.doctor_id != p.doctor_id
        || before.chair_id.as_deref() != chair
        || before.date != format_date(slot.date)
        || before.start_time != format_minutes(slot.start_min)
        || before.end_time != format_minutes(slot.end_min);
    if moved {
        if !matches!(before.status, Scheduled | Confirmed) {
            return Err(CoreError::invalid(
                "date",
                ValidationRule::NotEditable,
                "the time, doctor and chair cannot change after check-in",
            ));
        }
        check_doctor(conn, &p.doctor_id, before.doctor_id != p.doctor_id)?;
        check_chair(conn, &p.doctor_id, chair, before.chair_id.as_deref() != chair)?;
        check_schedule(conn, &p.doctor_id, &slot, p.override_schedule)?;
        check_conflicts(conn, &slot, &before.patient_id, &p.doctor_id, chair, Some(&p.id))?;
    }
    let changed = conn.execute(
        "UPDATE appointment SET doctor_id=?1, chair_id=?2, start_at=?3, end_at=?4, local_date=?5, start_min=?6, end_min=?7,
            reason=?8, notes=?9, updated_at=?10, updated_by=?11, version=version+1
         WHERE id=?12 AND version=?13 AND deleted_at IS NULL",
        params![
            p.doctor_id,
            chair,
            slot.start_at(),
            slot.end_at(),
            format_date(slot.date),
            slot.start_min,
            slot.end_min,
            clean(&p.reason),
            clean(&p.notes),
            now_iso(),
            actor.user_id,
            p.id,
            p.version
        ],
    )?;
    expect_one_row(changed, "appointment")?;
    let after = get_appointment(conn, &p.id)?;
    let action = if moved { "appointment.move" } else { "appointment.update" };
    audit::record(
        conn,
        actor,
        action,
        Some("appointment"),
        Some(&p.id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

/// Books a new slot and keeps the old appointment as `rescheduled`, linked both ways (4.4).
pub fn reschedule_appointment(
    conn: &Connection,
    actor: &Actor,
    p: &RescheduleAppointmentParams,
) -> Result<AppointmentInfo> {
    let slot = Slot::parse(&p.date, &p.start_time, &p.end_time)?;
    let before = get_appointment(conn, &p.id)?;
    if !matches!(before.status, AppointmentStatus::Scheduled | AppointmentStatus::Confirmed) {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::InvalidTransition,
            format!("a {} appointment cannot be rescheduled", before.status.code()),
        ));
    }
    let requested = p.chair_id.as_deref().filter(|s| !s.is_empty());
    check_doctor(conn, &p.doctor_id, true)?;
    check_chair(conn, &p.doctor_id, requested, true)?;
    check_not_past(&slot)?;
    check_schedule(conn, &p.doctor_id, &slot, p.override_schedule)?;
    // The old slot is freed by this very call, so it is excluded from the chair search and the conflict check.
    let chair = resolve_chair(conn, &p.doctor_id, requested, &slot, Some(&p.id))?;
    check_conflicts(conn, &slot, &before.patient_id, &p.doctor_id, chair.as_deref(), Some(&p.id))?;
    let new_id = insert(
        conn,
        actor,
        NewAppointment {
            patient_id: &before.patient_id,
            doctor_id: &p.doctor_id,
            chair_id: chair.as_deref(),
            slot,
            reason: before.reason.clone(),
            notes: before.notes.clone(),
            status: AppointmentStatus::Scheduled,
            is_walk_in: false,
            rescheduled_from_id: Some(&p.id),
        },
    )?;
    let now = now_iso();
    let changed = conn.execute(
        "UPDATE appointment SET status='rescheduled', rescheduled_to_id=?1, cancelled_at=?2,
            updated_at=?2, updated_by=?3, version=version+1
         WHERE id=?4 AND version=?5 AND deleted_at IS NULL",
        params![new_id, now, actor.user_id, p.id, p.version],
    )?;
    expect_one_row(changed, "appointment")?;
    recall::move_booking(conn, &p.id, &new_id)?;
    let after_old = get_appointment(conn, &p.id)?;
    let created = get_appointment(conn, &new_id)?;
    audit::record(
        conn,
        actor,
        "appointment.reschedule",
        Some("appointment"),
        Some(&p.id),
        Some(&json!(before)),
        Some(&json!({ "old": after_old, "new": created })),
    )?;
    Ok(created)
}

/// The statuses a visit may move to from `from` (4.4). `Rescheduled` is reached only through
/// [`reschedule_appointment`]. Backward steps undo a mistaken click at the front desk.
fn allowed(from: AppointmentStatus, to: AppointmentStatus) -> bool {
    use AppointmentStatus::*;
    matches!(
        (from, to),
        (Scheduled, Confirmed | CheckedIn | Cancelled | NoShow)
            | (Confirmed, CheckedIn | Cancelled | NoShow)
            | (CheckedIn, InTreatment | Cancelled | Confirmed)
            | (InTreatment, Completed | CheckedIn)
    )
}

pub fn set_status(
    conn: &Connection,
    actor: &Actor,
    p: &SetAppointmentStatusParams,
) -> Result<AppointmentInfo> {
    let before = get_appointment(conn, &p.id)?;
    let to = p.status;
    if !allowed(before.status, to) {
        return Err(CoreError::invalid(
            "status",
            ValidationRule::InvalidTransition,
            format!("cannot move an appointment from {} to {}", before.status.code(), to.code()),
        ));
    }
    if p.reason.as_deref().is_some_and(|r| r.trim().chars().count() > 200) {
        return Err(CoreError::invalid(
            "reason",
            ValidationRule::Required,
            "reason must be at most 200 characters",
        ));
    }
    let today = today_iso();
    if to == AppointmentStatus::CheckedIn
        && before.status != AppointmentStatus::InTreatment
        && before.date != today
    {
        return Err(CoreError::invalid(
            "status",
            ValidationRule::NotToday,
            "only today's appointments can be checked in",
        ));
    }

    use AppointmentStatus::*;
    let now = now_iso();
    let mut checked_in_at = before.checked_in_at.clone();
    let mut queue_number = before.queue_number;
    let mut treatment_started_at = before.treatment_started_at.clone();
    let mut completed_at = before.completed_at.clone();
    let mut cancelled_at = before.cancelled_at.clone();
    let mut cancel_reason = before.cancel_reason.clone();
    match (before.status, to) {
        (InTreatment, CheckedIn) => treatment_started_at = None,
        (_, CheckedIn) => {
            checked_in_at = Some(now.clone());
            queue_number = Some(next_queue_number(conn, &before.date)?);
        }
        (CheckedIn, Confirmed) => {
            checked_in_at = None;
            queue_number = None;
        }
        (_, InTreatment) => treatment_started_at = Some(now.clone()),
        (_, Completed) => completed_at = Some(now.clone()),
        (_, Cancelled | NoShow) => {
            cancelled_at = Some(now.clone());
            cancel_reason = clean(&p.reason);
        }
        _ => {}
    }
    let changed = conn.execute(
        "UPDATE appointment SET status=?1, checked_in_at=?2, queue_number=?3, treatment_started_at=?4,
            completed_at=?5, cancelled_at=?6, cancel_reason=?7, updated_at=?8, updated_by=?9, version=version+1
         WHERE id=?10 AND version=?11 AND deleted_at IS NULL",
        params![
            to.code(),
            checked_in_at,
            queue_number,
            treatment_started_at,
            completed_at,
            cancelled_at,
            cancel_reason,
            now,
            actor.user_id,
            p.id,
            p.version
        ],
    )?;
    expect_one_row(changed, "appointment")?;

    match to {
        Completed => {
            let on = crate::clock::local_now().date();
            recall::complete_for_appointment(conn, actor, &p.id, &before.patient_id, on)?;
            if let Some(f) = &p.follow_up {
                recall::create_follow_up(conn, actor, &before.patient_id, &p.id, f)?;
            }
        }
        Cancelled => recall::reopen_for_appointment(conn, actor, &p.id)?,
        NoShow => {
            recall::reopen_for_appointment(conn, actor, &p.id)?;
            recall::create_no_show_recall(conn, actor, &before.patient_id, &p.id, &today)?;
        }
        _ => {}
    }
    let after = get_appointment(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        &format!("appointment.{}", to.code()),
        Some("appointment"),
        Some(&p.id),
        Some(&json!({ "status": before.status })),
        Some(&json!(after)),
    )?;
    Ok(after)
}

/// A patient without an appointment (4.5): joins the queue right now, already checked in.
/// Walk-ins wait for the doctor, so they do not need a free slot and skip the schedule check;
/// a patient who is already waiting or in treatment cannot be added twice.
pub fn walk_in(conn: &Connection, actor: &Actor, p: &WalkInParams) -> Result<AppointmentInfo> {
    validate_texts(&p.reason, &None)?;
    let minutes = p.duration_minutes.map_or(DEFAULT_WALK_IN_MINUTES, i64::from);
    if !(MIN_MINUTES..=MAX_MINUTES).contains(&minutes) {
        return Err(CoreError::invalid(
            "duration_minutes",
            ValidationRule::DurationRange,
            "a visit lasts 5 minutes to 12 hours",
        ));
    }
    let requested = p.chair_id.as_deref().filter(|s| !s.is_empty());
    check_patient(conn, &p.patient_id)?;
    check_doctor(conn, &p.doctor_id, true)?;
    check_chair(conn, &p.doctor_id, requested, true)?;
    let today = today_iso();
    let in_queue: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM appointment WHERE patient_id = ?1 AND local_date = ?2 AND deleted_at IS NULL
                       AND status IN ('checked_in', 'in_treatment'))",
        params![p.patient_id, today],
        |r| r.get(0),
    )?;
    if in_queue {
        return Err(CoreError::invalid(
            "patient_id",
            ValidationRule::PatientBusy,
            "the patient is already in today's queue",
        ));
    }
    let now = local_now();
    let start_min = (now.hour() as i64 * 60 + now.minute() as i64).min(1435);
    let end_min = (start_min + minutes).min(1440);
    let slot = Slot { date: now.date(), start_min, end_min };
    // OF-037: a walk-in outside the doctor's working hours is refused until reception confirms
    // it (an emergency, say). A confirmed one is written to the audit log with that flag.
    check_schedule(conn, &p.doctor_id, &slot, p.override_schedule)?;
    let chair = resolve_chair(conn, &p.doctor_id, requested, &slot, None)?;
    let id = insert(
        conn,
        actor,
        NewAppointment {
            patient_id: &p.patient_id,
            doctor_id: &p.doctor_id,
            chair_id: chair.as_deref(),
            slot,
            reason: clean(&p.reason),
            notes: None,
            status: AppointmentStatus::CheckedIn,
            is_walk_in: true,
            rescheduled_from_id: None,
        },
    )?;
    let info = get_appointment(conn, &id)?;
    audit::record(
        conn,
        actor,
        "appointment.walk_in",
        Some("appointment"),
        Some(&id),
        None,
        Some(&json!({ "appointment": info, "outside_working_hours_confirmed": p.override_schedule })),
    )?;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use AppointmentStatus::*;

    #[test]
    fn status_workflow_matches_the_roadmap() {
        // The happy path.
        assert!(allowed(Scheduled, Confirmed));
        assert!(allowed(Confirmed, CheckedIn));
        assert!(allowed(CheckedIn, InTreatment));
        assert!(allowed(InTreatment, Completed));
        // Reception may skip "confirmed".
        assert!(allowed(Scheduled, CheckedIn));
        // Leaving the workflow.
        assert!(allowed(Scheduled, Cancelled) && allowed(Confirmed, NoShow) && allowed(CheckedIn, Cancelled));
        // No jumping ahead, no reviving finished visits, no manual "rescheduled".
        assert!(!allowed(Scheduled, InTreatment));
        assert!(!allowed(CheckedIn, Completed));
        assert!(!allowed(InTreatment, Cancelled));
        assert!(!allowed(Scheduled, Rescheduled) && !allowed(Confirmed, Rescheduled));
        for done in [Completed, Cancelled, NoShow, Rescheduled] {
            for to in [Scheduled, Confirmed, CheckedIn, InTreatment, Completed, Cancelled, NoShow] {
                assert!(!allowed(done, to), "{done:?} is final");
            }
        }
    }

    #[test]
    fn slots_are_validated_and_converted_to_utc() {
        let s = Slot::parse("2026-10-03", "09:00", "09:30").unwrap();
        // Kabul is UTC+04:30.
        assert_eq!(s.start_at(), "2026-10-03T04:30:00.000Z");
        assert_eq!(s.end_at(), "2026-10-03T05:00:00.000Z");
        // Early morning in Kabul is the previous day in UTC.
        assert_eq!(
            Slot::parse("2026-10-03", "02:00", "03:00").unwrap().start_at(),
            "2026-10-02T21:30:00.000Z"
        );
        assert!(Slot::parse("2026-10-03", "10:00", "09:00").is_err());
        assert!(Slot::parse("2026-10-03", "10:00", "10:04").is_err(), "shorter than 5 minutes");
        assert!(Slot::parse("2026-10-03", "00:00", "13:00").is_err(), "longer than 12 hours");
        assert!(Slot::parse("2026-02-30", "10:00", "11:00").is_err());
        assert!(Slot::parse("2026-10-03", "23:00", "24:00").is_ok(), "ends at midnight");
    }
}
