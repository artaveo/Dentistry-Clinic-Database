//! Treatment plans, treatments and multi-session courses (5.7, 5.8 and M4 of
//! docs/specs/clinical-workflow-v1.md).
//!
//! * A **plan** groups the procedures proposed to a patient; several proposed plans are alternatives.
//!   ```text
//!   Proposed → Accepted → In progress → Completed
//!          ↘ Cancelled (or set aside when the patient chose another plan)
//!   ```
//! * An **item** is one procedure: a catalog service on teeth/surfaces, a doctor, a price (the catalog's
//!   unless changed, AFN × 100), a phase and a number of sessions. Without a plan it is a treatment
//!   recorded directly (5.8). A completed item is a treatment done, ready for an invoice (Phase 6).
//! * A **session** is one visit's work on an item. Appointments (Phase 4) can be booked for "session 2
//!   of 3"; completing such a visit records the session, and the next one is suggested after the
//!   service's usual interval.
//! * Ready-made plans (`core/seeds/plan_templates.json`) are seeded once and then the clinic's.

use artaveo_shared::{
    AddItemParams, AppointmentInfo, CreatePlanParams, ErrorCode, ItemStatus, ItemVisit, Language,
    NextSessionSuggestion, PlanSnapshot, PlanSnapshotItem, PlanStatus, PlanTemplateInfo, PlanTemplateItem,
    RecordSessionParams, RecordSessionResult, SavePlanTemplateParams, ServiceInfo, SetItemStatusParams,
    SetPlanStatusParams, TemplateTeeth, ToothScope, Translations, TreatmentItemInfo, TreatmentPlanInfo,
    TreatmentSessionInfo, UpdateItemParams, UpdatePlanParams, ValidationRule,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::json;

use crate::audit::{self, Actor};
use crate::chart::{self, check_teeth, clean_surfaces, parse_teeth, teeth_text};
use crate::clock::{now_iso, today_iso};
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::formulary::clean_translations;
use crate::ids::{new_id, seed_id};
use crate::scheduling::{format_date, format_minutes, parse_date, LIVE_STATUSES_SQL};

const PLAN_TEMPLATES_JSON: &str = include_str!("../seeds/plan_templates.json");

/// Highest unit price accepted: 100 000 000 AFN (in AFN × 100), as in the catalog.
const MAX_PRICE: i64 = 10_000_000_000;

pub fn plan_status_code(s: PlanStatus) -> &'static str {
    match s {
        PlanStatus::Proposed => "proposed",
        PlanStatus::Accepted => "accepted",
        PlanStatus::InProgress => "in_progress",
        PlanStatus::Completed => "completed",
        PlanStatus::Cancelled => "cancelled",
    }
}

pub fn plan_status_from(s: &str) -> PlanStatus {
    match s {
        "accepted" => PlanStatus::Accepted,
        "in_progress" => PlanStatus::InProgress,
        "completed" => PlanStatus::Completed,
        "cancelled" => PlanStatus::Cancelled,
        _ => PlanStatus::Proposed,
    }
}

pub fn item_status_code(s: ItemStatus) -> &'static str {
    match s {
        ItemStatus::Planned => "planned",
        ItemStatus::InProgress => "in_progress",
        ItemStatus::Completed => "completed",
        ItemStatus::Cancelled => "cancelled",
    }
}

pub fn item_status_from(s: &str) -> ItemStatus {
    match s {
        "in_progress" => ItemStatus::InProgress,
        "completed" => ItemStatus::Completed,
        "cancelled" => ItemStatus::Cancelled,
        _ => ItemStatus::Planned,
    }
}

fn open(s: ItemStatus) -> bool {
    matches!(s, ItemStatus::Planned | ItemStatus::InProgress)
}

/// A plan still taking work: new items, edits, sessions.
fn plan_editable(s: PlanStatus) -> bool {
    matches!(s, PlanStatus::Proposed | PlanStatus::Accepted | PlanStatus::InProgress)
}

fn opt_text(field: &str, s: &Option<String>, max: usize) -> Result<Option<String>> {
    let v = s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if v.as_deref().is_some_and(|s| s.chars().count() > max) {
        return Err(CoreError::invalid(
            field,
            ValidationRule::DocumentText,
            format!("{field} is at most {max} characters"),
        ));
    }
    Ok(v)
}

fn opt_date(field: &'static str, s: &Option<String>) -> Result<Option<String>> {
    match s.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(d) => Ok(Some(format_date(parse_date(field, d)?))),
    }
}

/// The patient exists, is not deleted and was not merged into another record.
pub fn check_patient(conn: &Connection, patient_id: &str) -> Result<()> {
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

fn check_doctor(conn: &Connection, doctor_id: &Option<String>) -> Result<Option<String>> {
    match doctor_id.as_deref().filter(|d| !d.is_empty()) {
        None => Ok(None),
        Some(d) => {
            Ok(Some(crate::scheduling::get_doctor(conn, d).map_err(|e| e.rename_field("doctor_id"))?.id))
        }
    }
}

/// How many units a service is charged for on these teeth (M3 tooth scope).
pub fn units_for(scope: ToothScope, teeth: &[u8]) -> i64 {
    let distinct = |f: fn(u8) -> u8| {
        let mut v: Vec<u8> = teeth.iter().map(|&t| f(t)).collect();
        v.sort_unstable();
        v.dedup();
        v.len().max(1) as i64
    };
    match scope {
        ToothScope::Teeth => (teeth.len() as i64).clamp(1, 64),
        ToothScope::Quadrant => distinct(chart::quadrant),
        ToothScope::Arch => distinct(|t| chart::is_upper(t) as u8),
        ToothScope::Tooth | ToothScope::Mouth | ToothScope::None => 1,
    }
}

fn check_scope_teeth(scope: ToothScope, teeth: &[u8], single: bool) -> Result<()> {
    let needs =
        matches!(scope, ToothScope::Tooth | ToothScope::Teeth | ToothScope::Quadrant | ToothScope::Arch);
    if needs && teeth.is_empty() {
        return Err(CoreError::invalid(
            "teeth",
            ValidationRule::ItemTeeth,
            "choose the teeth for this service",
        ));
    }
    if single && scope == ToothScope::Tooth && teeth.len() != 1 {
        return Err(CoreError::invalid("teeth", ValidationRule::ItemTeeth, "this service is for one tooth"));
    }
    Ok(())
}

// ───────────────────────────── reads ─────────────────────────────

const ITEM_SELECT: &str =
    "SELECT i.id, i.plan_id, i.patient_id, i.service_id, i.teeth, i.surfaces, i.doctor_id,
    i.unit_price, i.units, i.phase, i.status, i.sessions_planned, i.details, i.notes, i.est_start, i.est_end,
    i.completed_at, i.cancel_reason, i.billed_at, i.sort_order, i.created_at, i.version
    FROM treatment_item i WHERE i.deleted_at IS NULL";

fn map_item(r: &rusqlite::Row<'_>) -> rusqlite::Result<TreatmentItemInfo> {
    Ok(TreatmentItemInfo {
        id: r.get(0)?,
        plan_id: r.get(1)?,
        patient_id: r.get(2)?,
        service_id: r.get(3)?,
        teeth: parse_teeth(&r.get::<_, String>(4)?),
        surfaces: r.get(5)?,
        doctor_id: r.get(6)?,
        unit_price: r.get(7)?,
        units: r.get(8)?,
        phase: r.get(9)?,
        status: item_status_from(&r.get::<_, String>(10)?),
        sessions_planned: r.get(11)?,
        sessions_done: 0,
        sessions: Vec::new(),
        upcoming: Vec::new(),
        details: r.get(12)?,
        notes: r.get(13)?,
        est_start: r.get(14)?,
        est_end: r.get(15)?,
        completed_at: r.get(16)?,
        cancel_reason: r.get(17)?,
        billed: r.get::<_, Option<String>>(18)?.is_some(),
        sort_order: r.get(19)?,
        created_at: r.get(20)?,
        version: r.get(21)?,
    })
}

fn sessions_of(conn: &Connection, item_id: &str) -> Result<Vec<TreatmentSessionInfo>> {
    let mut stmt = conn.prepare_cached(
        "SELECT s.id, s.item_id, s.session_no, s.appointment_id, s.doctor_id, d.full_name, s.performed_on, s.note,
            u.display_name
         FROM treatment_session s LEFT JOIN doctor d ON d.id = s.doctor_id LEFT JOIN app_user u ON u.id = s.created_by
         WHERE s.item_id = ?1 ORDER BY s.session_no, s.created_at",
    )?;
    let v = stmt
        .query_map([item_id], |r| {
            Ok(TreatmentSessionInfo {
                id: r.get(0)?,
                item_id: r.get(1)?,
                session_no: r.get(2)?,
                appointment_id: r.get(3)?,
                doctor_id: r.get(4)?,
                doctor_name: r.get(5)?,
                performed_on: r.get(6)?,
                note: r.get(7)?,
                created_by_name: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

fn upcoming_of(conn: &Connection, item_id: &str) -> Result<Vec<ItemVisit>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT id, local_date, start_min, session_no FROM appointment
         WHERE treatment_item_id = ?1 AND deleted_at IS NULL AND status IN {LIVE_STATUSES_SQL} ORDER BY start_at"
    ))?;
    let v = stmt
        .query_map([item_id], |r| {
            Ok(ItemVisit {
                appointment_id: r.get(0)?,
                date: r.get(1)?,
                start_time: format_minutes(r.get(2)?),
                session_no: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

fn fill(conn: &Connection, mut i: TreatmentItemInfo) -> Result<TreatmentItemInfo> {
    i.sessions = sessions_of(conn, &i.id)?;
    i.sessions_done = i.sessions.len() as i64;
    i.upcoming = if open(i.status) { upcoming_of(conn, &i.id)? } else { Vec::new() };
    Ok(i)
}

fn items_where(
    conn: &Connection,
    filter: &str,
    args: &[&dyn rusqlite::ToSql],
) -> Result<Vec<TreatmentItemInfo>> {
    let mut stmt = conn.prepare(&format!("{ITEM_SELECT} {filter}"))?;
    let rows: Vec<TreatmentItemInfo> = stmt.query_map(args, map_item)?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|i| fill(conn, i)).collect()
}

pub fn get_item(conn: &Connection, id: &str) -> Result<TreatmentItemInfo> {
    let i = conn.query_row(&format!("{ITEM_SELECT} AND i.id = ?1"), [id], map_item).optional()?.ok_or_else(
        || {
            CoreError::api(ErrorCode::NotFound, format!("treatment item {id}"))
                .on_field("id", ValidationRule::ItemNotFound)
        },
    )?;
    fill(conn, i)
}

/// Every item the chart draws: all but cancelled ones.
pub fn chart_items(conn: &Connection, patient_id: &str) -> Result<Vec<TreatmentItemInfo>> {
    items_where(
        conn,
        "AND i.patient_id = ?1 AND i.status <> 'cancelled' ORDER BY i.created_at",
        &[&patient_id],
    )
}

/// Items still to do, for booking their sessions (reception): open items of plans still running.
pub fn open_items(conn: &Connection, patient_id: &str) -> Result<Vec<TreatmentItemInfo>> {
    items_where(
        conn,
        "AND i.patient_id = ?1 AND i.status IN ('planned', 'in_progress')
         AND (i.plan_id IS NULL OR EXISTS(SELECT 1 FROM treatment_plan p WHERE p.id = i.plan_id
                                          AND p.status IN ('proposed', 'accepted', 'in_progress')))
         ORDER BY i.phase, i.created_at",
        &[&patient_id],
    )
}

/// Treatments done (5.8): completed items and items with at least one session, latest first.
pub fn treatments(conn: &Connection, patient_id: &str) -> Result<Vec<TreatmentItemInfo>> {
    let mut v: Vec<TreatmentItemInfo> = items_where(conn, "AND i.patient_id = ?1", &[&patient_id])?
        .into_iter()
        .filter(|i| i.status == ItemStatus::Completed || i.sessions_done > 0)
        .collect();
    let last = |i: &TreatmentItemInfo| {
        i.sessions
            .iter()
            .map(|s| s.performed_on.clone())
            .max()
            .into_iter()
            .chain(i.completed_at.clone())
            .max()
    };
    v.sort_by_key(|i| std::cmp::Reverse(last(i)));
    Ok(v)
}

const PLAN_SELECT: &str =
    "SELECT p.id, p.patient_id, p.title, p.status, p.notes, p.created_at, u.display_name,
    p.accepted_at, p.completed_at, p.cancelled_at, p.cancel_reason, p.superseded_by, p.version
    FROM treatment_plan p LEFT JOIN app_user u ON u.id = p.created_by
    WHERE p.deleted_at IS NULL";

fn map_plan(r: &rusqlite::Row<'_>) -> rusqlite::Result<TreatmentPlanInfo> {
    Ok(TreatmentPlanInfo {
        id: r.get(0)?,
        patient_id: r.get(1)?,
        title: r.get(2)?,
        status: plan_status_from(&r.get::<_, String>(3)?),
        notes: r.get(4)?,
        created_at: r.get(5)?,
        created_by_name: r.get(6)?,
        accepted_at: r.get(7)?,
        completed_at: r.get(8)?,
        cancelled_at: r.get(9)?,
        cancel_reason: r.get(10)?,
        superseded_by: r.get(11)?,
        items: Vec::new(),
        total: 0,
        version: r.get(12)?,
    })
}

fn with_items(conn: &Connection, mut p: TreatmentPlanInfo) -> Result<TreatmentPlanInfo> {
    p.items = items_where(conn, "AND i.plan_id = ?1 ORDER BY i.phase, i.sort_order, i.created_at", &[&p.id])?;
    p.total =
        p.items.iter().filter(|i| i.status != ItemStatus::Cancelled).map(|i| i.unit_price * i.units).sum();
    Ok(p)
}

pub fn get_plan(conn: &Connection, id: &str) -> Result<TreatmentPlanInfo> {
    let p = conn.query_row(&format!("{PLAN_SELECT} AND p.id = ?1"), [id], map_plan).optional()?.ok_or_else(
        || {
            CoreError::api(ErrorCode::NotFound, format!("treatment plan {id}"))
                .on_field("plan_id", ValidationRule::PlanNotFound)
        },
    )?;
    with_items(conn, p)
}

/// A patient's plans: the ones still running first, then finished and cancelled ones, newest first.
pub fn list_plans(conn: &Connection, patient_id: &str) -> Result<Vec<TreatmentPlanInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{PLAN_SELECT} AND p.patient_id = ?1
         ORDER BY CASE p.status WHEN 'in_progress' THEN 0 WHEN 'accepted' THEN 1 WHEN 'proposed' THEN 2
                  WHEN 'completed' THEN 3 ELSE 4 END, p.created_at DESC"
    ))?;
    let rows: Vec<TreatmentPlanInfo> =
        stmt.query_map([patient_id], map_plan)?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|p| with_items(conn, p)).collect()
}

// ───────────────────────────── plans ─────────────────────────────

fn plan_title(t: &str) -> Result<String> {
    let t = t.trim();
    if t.is_empty() {
        return Err(CoreError::invalid("title", ValidationRule::Required, "a plan needs a title"));
    }
    if t.chars().count() > 120 {
        return Err(CoreError::invalid(
            "title",
            ValidationRule::DocumentText,
            "a title is at most 120 characters",
        ));
    }
    Ok(t.to_string())
}

pub fn create_plan(conn: &Connection, actor: &Actor, p: &CreatePlanParams) -> Result<TreatmentPlanInfo> {
    check_patient(conn, &p.patient_id)?;
    let title = plan_title(&p.title)?;
    let notes = opt_text("notes", &p.notes, 2000)?;
    let template = match p.template_id.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => Some(get_plan_template(conn, t).map_err(|e| e.rename_field("template_id"))?),
        None => None,
    };
    let id = new_id();
    let now = now_iso();
    conn.execute(
        "INSERT INTO treatment_plan(id, patient_id, title, notes, created_at, created_by, updated_at, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5, ?6)",
        params![id, p.patient_id, title, notes, now, actor.user_id],
    )?;
    audit::record(
        conn,
        actor,
        "plan.create",
        Some("patient"),
        Some(&p.patient_id),
        None,
        Some(&json!({"plan_id": id, "title": title, "template": template.as_ref().map(|t| &t.code)})),
    )?;
    if let Some(t) = template {
        apply_template(conn, actor, &p.patient_id, &id, &t, &p.teeth)?;
    }
    get_plan(conn, &id)
}

/// A ready-made plan's lines on this patient: tooth lines on the selected teeth, jaw lines on that jaw.
fn apply_template(
    conn: &Connection,
    actor: &Actor,
    patient_id: &str,
    plan_id: &str,
    t: &PlanTemplateInfo,
    selected: &[u8],
) -> Result<()> {
    let (dentition, slots, _, _) = chart::dentition_of(conn, patient_id)?;
    let all = chart::all_teeth(dentition, &slots);
    let selected = check_teeth("teeth", selected)?;
    for line in &t.items {
        let Ok(service) = crate::catalog::get_service(conn, &line.service_id) else { continue };
        if !service.is_active {
            continue;
        }
        let teeth: Vec<u8> = match line.teeth {
            TemplateTeeth::Selected => selected.clone(),
            TemplateTeeth::Upper => all.iter().copied().filter(|&x| chart::is_upper(x)).collect(),
            TemplateTeeth::Lower => all.iter().copied().filter(|&x| !chart::is_upper(x)).collect(),
            TemplateTeeth::All => all.clone(),
            TemplateTeeth::None => Vec::new(),
        };
        add_items(
            conn,
            actor,
            &AddItemParams {
                patient_id: patient_id.to_string(),
                plan_id: Some(plan_id.to_string()),
                service_id: service.id.clone(),
                teeth,
                surfaces: String::new(),
                doctor_id: None,
                unit_price: None,
                phase: Some(line.phase),
                sessions: None,
                details: None,
                notes: None,
                est_start: None,
                est_end: None,
                perform_now: false,
                appointment_id: None,
            },
        )?;
    }
    Ok(())
}

pub fn update_plan(conn: &Connection, actor: &Actor, p: &UpdatePlanParams) -> Result<TreatmentPlanInfo> {
    let before = get_plan(conn, &p.id).map_err(|e| e.rename_field("id"))?;
    if before.status == PlanStatus::Cancelled {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::PlanNotEditable,
            "a cancelled plan cannot change",
        ));
    }
    let title = plan_title(&p.title)?;
    let notes = opt_text("notes", &p.notes, 2000)?;
    let changed = conn.execute(
        "UPDATE treatment_plan SET title=?1, notes=?2, updated_at=?3, updated_by=?4, version=version+1
         WHERE id=?5 AND version=?6 AND deleted_at IS NULL",
        params![title, notes, now_iso(), actor.user_id, p.id, p.version],
    )?;
    expect_one_row(changed, "treatment plan")?;
    audit::record(
        conn,
        actor,
        "plan.update",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!({"plan_id": p.id, "title": before.title, "notes": before.notes})),
        Some(&json!({"title": title, "notes": notes})),
    )?;
    get_plan(conn, &p.id)
}

/// Cancels a plan's items that were not started (the plan was cancelled or set aside).
fn cancel_planned_items(conn: &Connection, actor: &Actor, plan_id: &str, now: &str) -> Result<()> {
    conn.execute(
        "UPDATE treatment_item SET status='cancelled', cancelled_at=?1, updated_at=?1, updated_by=?2, version=version+1
         WHERE plan_id=?3 AND status='planned' AND deleted_at IS NULL",
        params![now, actor.user_id, plan_id],
    )?;
    conn.execute(
        &format!(
            "UPDATE appointment SET treatment_item_id=NULL, session_no=NULL
             WHERE status IN {LIVE_STATUSES_SQL} AND treatment_item_id IN
                   (SELECT id FROM treatment_item WHERE plan_id=?1 AND status='cancelled')"
        ),
        [plan_id],
    )?;
    Ok(())
}

pub fn set_plan_status(
    conn: &Connection,
    actor: &Actor,
    p: &SetPlanStatusParams,
) -> Result<TreatmentPlanInfo> {
    use PlanStatus::*;
    let before = get_plan(conn, &p.id).map_err(|e| e.rename_field("id"))?;
    let reason = opt_text("reason", &p.reason, 300)?;
    let now = now_iso();
    let has_work = before.items.iter().any(|i| i.status == ItemStatus::InProgress);
    let has_open = before.items.iter().any(|i| open(i.status));
    let any_session = before.items.iter().any(|i| i.sessions_done > 0);
    let (sql, args): (&str, Vec<Option<String>>) = match (before.status, p.status) {
        (Proposed, Accepted) => ("status='accepted', accepted_at=?1", vec![Some(now.clone())]),
        (Accepted, Proposed) if !any_session => ("status='proposed', accepted_at=NULL", vec![]),
        (Proposed | Accepted | InProgress, Cancelled) => {
            if has_work {
                return Err(CoreError::invalid(
                    "status",
                    ValidationRule::PlanHasWorkInProgress,
                    "finish or cancel the items in progress first",
                ));
            }
            ("status='cancelled', cancelled_at=?1, cancel_reason=?2", vec![Some(now.clone()), reason.clone()])
        }
        (Accepted | InProgress, Completed) => {
            if has_open {
                return Err(CoreError::invalid(
                    "status",
                    ValidationRule::PlanHasWorkInProgress,
                    "some items are not finished",
                ));
            }
            ("status='completed', completed_at=?1", vec![Some(now.clone())])
        }
        (Cancelled, Proposed) => {
            ("status='proposed', cancelled_at=NULL, cancel_reason=NULL, superseded_by=NULL", vec![])
        }
        (from, to) => {
            return Err(CoreError::invalid(
                "status",
                ValidationRule::InvalidTransition,
                format!("a plan cannot go from {} to {}", plan_status_code(from), plan_status_code(to)),
            ))
        }
    };
    let n = args.len();
    let mut values: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a as &dyn rusqlite::ToSql).collect();
    let tail = [&now as &dyn rusqlite::ToSql, &actor.user_id, &p.id, &p.version];
    values.extend(tail);
    let changed = conn.execute(
        &format!(
            "UPDATE treatment_plan SET {sql}, updated_at=?{}, updated_by=?{}, version=version+1
             WHERE id=?{} AND version=?{} AND deleted_at IS NULL",
            n + 1,
            n + 2,
            n + 3,
            n + 4
        ),
        values.as_slice(),
    )?;
    expect_one_row(changed, "treatment plan")?;
    if p.status == Cancelled {
        cancel_planned_items(conn, actor, &p.id, &now)?;
    }
    let mut set_aside = Vec::new();
    if p.status == Accepted && p.cancel_alternatives {
        let mut stmt = conn.prepare(
            "SELECT id FROM treatment_plan WHERE patient_id=?1 AND id<>?2 AND status='proposed' AND deleted_at IS NULL",
        )?;
        let others: Vec<String> = stmt
            .query_map(params![before.patient_id, p.id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for other in others {
            conn.execute(
                "UPDATE treatment_plan SET status='cancelled', cancelled_at=?1, superseded_by=?2, updated_at=?1,
                    updated_by=?3, version=version+1 WHERE id=?4",
                params![now, p.id, actor.user_id, other],
            )?;
            cancel_planned_items(conn, actor, &other, &now)?;
            set_aside.push(other);
        }
    }
    let after = get_plan(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        &format!("plan.{}", plan_status_code(p.status)),
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!({"plan_id": p.id, "status": plan_status_code(before.status)})),
        Some(&json!({"status": plan_status_code(after.status), "reason": reason, "set_aside": set_aside})),
    )?;
    Ok(after)
}

/// Keeps a plan's status in step with its items: work started → in progress; every item done → completed.
fn refresh_plan(conn: &Connection, plan_id: Option<&str>) -> Result<()> {
    let Some(plan_id) = plan_id else { return Ok(()) };
    let plan = get_plan(conn, plan_id)?;
    let now = now_iso();
    let live: Vec<&TreatmentItemInfo> =
        plan.items.iter().filter(|i| i.status != ItemStatus::Cancelled).collect();
    let started = live.iter().any(|i| i.sessions_done > 0 || i.status == ItemStatus::Completed);
    let mut status = plan.status;
    if started && matches!(status, PlanStatus::Proposed | PlanStatus::Accepted) {
        conn.execute(
            "UPDATE treatment_plan SET status='in_progress', accepted_at=COALESCE(accepted_at, ?1), updated_at=?1,
                version=version+1 WHERE id=?2",
            params![now, plan_id],
        )?;
        status = PlanStatus::InProgress;
    }
    let all_done = !live.is_empty() && live.iter().all(|i| i.status == ItemStatus::Completed);
    if all_done && matches!(status, PlanStatus::Accepted | PlanStatus::InProgress) {
        conn.execute(
            "UPDATE treatment_plan SET status='completed', completed_at=?1, updated_at=?1, version=version+1 WHERE id=?2",
            params![now, plan_id],
        )?;
    } else if !all_done && status == PlanStatus::Completed {
        // An item put back to work reopens its plan.
        conn.execute(
            "UPDATE treatment_plan SET status='in_progress', completed_at=NULL, updated_at=?1, version=version+1 WHERE id=?2",
            params![now, plan_id],
        )?;
    }
    Ok(())
}

// ───────────────────────────── items ─────────────────────────────

pub fn add_items(conn: &Connection, actor: &Actor, p: &AddItemParams) -> Result<Vec<TreatmentItemInfo>> {
    check_patient(conn, &p.patient_id)?;
    let plan_id = match p.plan_id.as_deref().filter(|s| !s.is_empty()) {
        Some(id) => {
            let plan = get_plan(conn, id)?;
            if plan.patient_id != p.patient_id {
                return Err(CoreError::invalid(
                    "plan_id",
                    ValidationRule::ItemPatient,
                    "that plan is another patient's",
                ));
            }
            if !plan_editable(plan.status) {
                return Err(CoreError::invalid(
                    "plan_id",
                    ValidationRule::PlanNotEditable,
                    "this plan is closed",
                ));
            }
            Some(plan.id)
        }
        None => None,
    };
    let service =
        crate::catalog::get_service(conn, &p.service_id).map_err(|e| e.rename_field("service_id"))?;
    if !service.is_active {
        return Err(CoreError::invalid(
            "service_id",
            ValidationRule::ServiceNotFound,
            "this service is switched off",
        ));
    }
    let teeth = check_teeth("teeth", &p.teeth)?;
    check_scope_teeth(service.tooth_scope, &teeth, false)?;
    let surfaces = clean_surfaces("surfaces", &p.surfaces)?;
    if service.needs_surface && service.tooth_scope == ToothScope::Tooth && surfaces.is_empty() {
        return Err(CoreError::invalid("surfaces", ValidationRule::ToothSurface, "choose the surfaces"));
    }
    let doctor = check_doctor(conn, &p.doctor_id)?;
    let unit_price = p.unit_price.unwrap_or(service.price);
    if !(0..=MAX_PRICE).contains(&unit_price) {
        return Err(CoreError::invalid("unit_price", ValidationRule::ItemPrice, "the price is out of range"));
    }
    let phase = p.phase.unwrap_or(1);
    if !(1..=5).contains(&phase) {
        return Err(CoreError::invalid("phase", ValidationRule::ItemPhase, "phase 1 to 5"));
    }
    let sessions = p.sessions.unwrap_or(service.sessions);
    if !(1..=60).contains(&sessions) {
        return Err(CoreError::invalid("sessions", ValidationRule::ItemSessions, "1 to 60 sessions"));
    }
    let details = opt_text("details", &p.details, 80)?;
    let notes = opt_text("notes", &p.notes, 1000)?;
    let est_start = opt_date("est_start", &p.est_start)?;
    let est_end = opt_date("est_end", &p.est_end)?;
    // A one-tooth service on several teeth is one item per tooth (each its own sessions and price).
    let groups: Vec<Vec<u8>> = if service.tooth_scope == ToothScope::Tooth {
        teeth.iter().map(|&t| vec![t]).collect()
    } else {
        vec![teeth.clone()]
    };
    let now = now_iso();
    let mut sort: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) FROM treatment_item WHERE patient_id = ?1 AND plan_id IS ?2",
        params![p.patient_id, plan_id],
        |r| r.get(0),
    )?;
    let mut ids = Vec::new();
    for g in groups {
        sort += 10;
        let id = new_id();
        conn.execute(
            "INSERT INTO treatment_item(id, plan_id, patient_id, service_id, teeth, surfaces, doctor_id, unit_price, units,
                phase, sessions_planned, details, notes, est_start, est_end, sort_order, created_at, created_by, updated_at,
                updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?17, ?18)",
            params![
                id,
                plan_id,
                p.patient_id,
                service.id,
                teeth_text(&g),
                surfaces,
                doctor,
                unit_price,
                units_for(service.tooth_scope, &g),
                phase,
                sessions,
                details,
                notes,
                est_start,
                est_end,
                sort,
                now,
                actor.user_id
            ],
        )?;
        ids.push(id);
    }
    audit::record(
        conn,
        actor,
        "treatment_item.create",
        Some("patient"),
        Some(&p.patient_id),
        None,
        Some(
            &json!({"items": ids, "plan_id": plan_id, "service": service.code, "teeth": teeth, "surfaces": surfaces,
            "unit_price": unit_price, "catalog_price": service.price}),
        ),
    )?;
    if p.perform_now {
        let appt = p.appointment_id.as_deref().filter(|a| !a.is_empty());
        for id in &ids {
            insert_session(conn, actor, id, appt, doctor.as_deref(), None, None)?;
        }
    }
    ids.iter().map(|id| get_item(conn, id)).collect()
}

pub fn update_item(conn: &Connection, actor: &Actor, p: &UpdateItemParams) -> Result<TreatmentItemInfo> {
    let before = get_item(conn, &p.id).map_err(|e| e.rename_field("id"))?;
    if !open(before.status) {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::ItemNotEditable,
            "a finished item cannot change",
        ));
    }
    if let Some(pid) = &before.plan_id {
        if !plan_editable(get_plan(conn, pid)?.status) {
            return Err(CoreError::invalid("id", ValidationRule::PlanNotEditable, "this plan is closed"));
        }
    }
    let service = crate::catalog::get_service(conn, &before.service_id)?;
    let teeth = check_teeth("teeth", &p.teeth)?;
    check_scope_teeth(service.tooth_scope, &teeth, true)?;
    let surfaces = clean_surfaces("surfaces", &p.surfaces)?;
    if service.needs_surface && service.tooth_scope == ToothScope::Tooth && surfaces.is_empty() {
        return Err(CoreError::invalid("surfaces", ValidationRule::ToothSurface, "choose the surfaces"));
    }
    let doctor = check_doctor(conn, &p.doctor_id)?;
    if !(0..=MAX_PRICE).contains(&p.unit_price) {
        return Err(CoreError::invalid("unit_price", ValidationRule::ItemPrice, "the price is out of range"));
    }
    if !(1..=5).contains(&p.phase) {
        return Err(CoreError::invalid("phase", ValidationRule::ItemPhase, "phase 1 to 5"));
    }
    if !(1..=60).contains(&p.sessions_planned) || p.sessions_planned < before.sessions_done {
        return Err(CoreError::invalid(
            "sessions_planned",
            ValidationRule::ItemSessions,
            "1 to 60 sessions, and not fewer than already done",
        ));
    }
    let details = opt_text("details", &p.details, 80)?;
    let notes = opt_text("notes", &p.notes, 1000)?;
    let est_start = opt_date("est_start", &p.est_start)?;
    let est_end = opt_date("est_end", &p.est_end)?;
    let now = now_iso();
    // Lowering the sessions to those already done finishes the item.
    let finished = before.sessions_done > 0 && before.sessions_done >= p.sessions_planned;
    let changed = conn.execute(
        "UPDATE treatment_item SET teeth=?1, surfaces=?2, doctor_id=?3, unit_price=?4, units=?5, phase=?6,
            sessions_planned=?7, details=?8, notes=?9, est_start=?10, est_end=?11, updated_at=?12, updated_by=?13,
            version=version+1, status=CASE WHEN ?16 THEN 'completed' ELSE status END,
            completed_at=CASE WHEN ?16 THEN ?12 ELSE completed_at END
         WHERE id=?14 AND version=?15 AND deleted_at IS NULL",
        params![
            teeth_text(&teeth),
            surfaces,
            doctor,
            p.unit_price,
            units_for(service.tooth_scope, &teeth),
            p.phase,
            p.sessions_planned,
            details,
            notes,
            est_start,
            est_end,
            now,
            actor.user_id,
            p.id,
            p.version,
            finished
        ],
    )?;
    expect_one_row(changed, "treatment item")?;
    refresh_plan(conn, before.plan_id.as_deref())?;
    let after = get_item(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        "treatment_item.update",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

pub fn set_item_status(
    conn: &Connection,
    actor: &Actor,
    p: &SetItemStatusParams,
) -> Result<TreatmentItemInfo> {
    use ItemStatus::*;
    let before = get_item(conn, &p.id).map_err(|e| e.rename_field("id"))?;
    let reason = opt_text("reason", &p.reason, 300)?;
    let now = now_iso();
    // (status, completed_at, cancelled_at, cancel_reason) after the move.
    let (status, completed_at, cancelled_at, cancel_reason) = match (before.status, p.status) {
        (Planned | InProgress, Completed) => (Completed, Some(now.clone()), None, None),
        (Planned | InProgress, Cancelled) => (Cancelled, None, Some(now.clone()), reason.clone()),
        (Cancelled, Planned) => {
            if let Some(pid) = &before.plan_id {
                if !plan_editable(get_plan(conn, pid)?.status) {
                    return Err(CoreError::invalid(
                        "id",
                        ValidationRule::PlanNotEditable,
                        "this plan is closed",
                    ));
                }
            }
            (if before.sessions_done > 0 { InProgress } else { Planned }, None, None, None)
        }
        (from, to) => {
            return Err(CoreError::invalid(
                "status",
                ValidationRule::InvalidTransition,
                format!("an item cannot go from {} to {}", item_status_code(from), item_status_code(to)),
            ))
        }
    };
    let changed = conn.execute(
        "UPDATE treatment_item SET status=?1, completed_at=?2, cancelled_at=?3, cancel_reason=?4, updated_at=?5,
            updated_by=?6, version=version+1
         WHERE id=?7 AND version=?8 AND deleted_at IS NULL",
        params![item_status_code(status), completed_at, cancelled_at, cancel_reason, now, actor.user_id, p.id, p.version],
    )?;
    expect_one_row(changed, "treatment item")?;
    if p.status == Cancelled {
        // Its booked visits stay as ordinary visits.
        conn.execute(
            &format!(
                "UPDATE appointment SET treatment_item_id=NULL, session_no=NULL
                 WHERE treatment_item_id=?1 AND status IN {LIVE_STATUSES_SQL}"
            ),
            [&p.id],
        )?;
    }
    refresh_plan(conn, before.plan_id.as_deref())?;
    let after = get_item(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        &format!("treatment_item.{}", item_status_code(p.status)),
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!({"item_id": p.id, "status": item_status_code(before.status)})),
        Some(&json!({"status": item_status_code(after.status), "reason": reason})),
    )?;
    Ok(after)
}

/// Records the next session of an item and moves it (and its plan) on. Returns the session.
fn insert_session(
    conn: &Connection,
    actor: &Actor,
    item_id: &str,
    appointment_id: Option<&str>,
    doctor_id: Option<&str>,
    performed_on: Option<String>,
    note: Option<String>,
) -> Result<TreatmentSessionInfo> {
    let item = get_item(conn, item_id)?;
    if !open(item.status) {
        return Err(CoreError::invalid(
            "item_id",
            ValidationRule::ItemNotEditable,
            "this item is finished or cancelled",
        ));
    }
    if let Some(pid) = &item.plan_id {
        if !plan_editable(get_plan(conn, pid)?.status) {
            return Err(CoreError::invalid(
                "item_id",
                ValidationRule::PlanNotEditable,
                "this plan is closed",
            ));
        }
    }
    let mut doctor = doctor_id.map(str::to_string);
    if let Some(a) = appointment_id {
        let appt =
            crate::appointment::get_appointment(conn, a).map_err(|e| e.rename_field("appointment_id"))?;
        if appt.patient_id != item.patient_id {
            return Err(CoreError::invalid(
                "appointment_id",
                ValidationRule::ItemPatient,
                "that visit is another patient's",
            ));
        }
        let already: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM treatment_session WHERE item_id = ?1 AND appointment_id = ?2)",
            params![item_id, a],
            |r| r.get(0),
        )?;
        if already {
            return Err(CoreError::invalid(
                "appointment_id",
                ValidationRule::SessionNumber,
                "a session was already recorded for this visit",
            ));
        }
        doctor = doctor.or(Some(appt.doctor_id));
    }
    let doctor = doctor.or_else(|| item.doctor_id.clone());
    let today = today_iso();
    let performed_on = match performed_on {
        Some(d) => format_date(parse_date("performed_on", &d)?),
        None => today.clone(),
    };
    if performed_on > today {
        return Err(CoreError::invalid(
            "performed_on",
            ValidationRule::DateFormat,
            "a session cannot be in the future",
        ));
    }
    let session_no = item.sessions_done + 1;
    if session_no > 99 {
        return Err(CoreError::invalid("item_id", ValidationRule::SessionNumber, "too many sessions"));
    }
    let id = new_id();
    let now = now_iso();
    conn.execute(
        "INSERT INTO treatment_session(id, item_id, patient_id, session_no, appointment_id, doctor_id, performed_on, note,
            created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, item_id, item.patient_id, session_no, appointment_id, doctor, performed_on, note, now, actor.user_id],
    )?;
    let done = session_no >= item.sessions_planned;
    conn.execute(
        "UPDATE treatment_item SET status=?1, completed_at=CASE WHEN ?1 = 'completed' THEN ?2 ELSE completed_at END,
            doctor_id=COALESCE(doctor_id, ?3), updated_at=?2, updated_by=?4, version=version+1
         WHERE id=?5",
        params![if done { "completed" } else { "in_progress" }, now, doctor, actor.user_id, item_id],
    )?;
    refresh_plan(conn, item.plan_id.as_deref())?;
    audit::record(
        conn,
        actor,
        "treatment_item.session",
        Some("patient"),
        Some(&item.patient_id),
        None,
        Some(
            &json!({"item_id": item_id, "session_no": session_no, "sessions_planned": item.sessions_planned,
            "appointment_id": appointment_id, "performed_on": performed_on, "completed": done}),
        ),
    )?;
    Ok(sessions_of(conn, item_id)?.into_iter().find(|s| s.id == id).expect("just inserted"))
}

pub fn record_session(
    conn: &Connection,
    actor: &Actor,
    p: &RecordSessionParams,
) -> Result<RecordSessionResult> {
    let item = get_item(conn, &p.item_id).map_err(|e| e.rename_field("item_id"))?;
    if item.version != p.version {
        return Err(CoreError::api(ErrorCode::Conflict, "the item was changed"));
    }
    let doctor = check_doctor(conn, &p.doctor_id)?;
    let note = opt_text("note", &p.note, 500)?;
    let session = insert_session(
        conn,
        actor,
        &p.item_id,
        p.appointment_id.as_deref().filter(|a| !a.is_empty()),
        doctor.as_deref(),
        p.performed_on.clone().filter(|d| !d.trim().is_empty()),
        note,
    )?;
    let item = get_item(conn, &p.item_id)?;
    let next = suggest_next(conn, &p.item_id)?;
    Ok(RecordSessionResult { item, session, next })
}

/// When to book the next session: after the last one (or today) plus the service's usual interval.
/// None when the item is finished, its sessions are all done, or the next one is already booked.
pub fn suggest_next(conn: &Connection, item_id: &str) -> Result<Option<NextSessionSuggestion>> {
    let item = get_item(conn, item_id).map_err(|e| e.rename_field("item_id"))?;
    if !open(item.status) || !item.upcoming.is_empty() {
        return Ok(None);
    }
    let session_no = item.sessions_done + 1;
    if session_no > item.sessions_planned {
        return Ok(None);
    }
    let service = crate::catalog::get_service(conn, &item.service_id)?;
    let today = parse_date("today", &today_iso())?;
    let last = item.sessions.last();
    let date = match last {
        Some(s) => {
            parse_date("performed_on", &s.performed_on)? + time::Duration::days(service.session_interval_days)
        }
        None => today + time::Duration::days(1),
    };
    let date = date.max(today + time::Duration::days(1));
    // As long as the item's last visit, else half an hour.
    let duration: Option<i64> = conn
        .query_row(
            "SELECT end_min - start_min FROM appointment WHERE treatment_item_id = ?1 AND deleted_at IS NULL
             ORDER BY start_at DESC LIMIT 1",
            [item_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(Some(NextSessionSuggestion {
        item_id: item.id.clone(),
        session_no,
        sessions_planned: item.sessions_planned,
        date: format_date(date),
        interval_days: service.session_interval_days,
        duration_minutes: duration.unwrap_or(30),
        doctor_id: item.doctor_id.clone().or_else(|| last.and_then(|s| s.doctor_id.clone())),
    }))
}

// ───────────────────────────── appointments (M4) ─────────────────────────────

/// Checks that a visit may be booked as a session of `item_id` and picks the session number: the
/// next one neither done nor already booked unless given. Returns (item id, session no, service id).
pub fn check_link(
    conn: &Connection,
    patient_id: &str,
    item_id: Option<&str>,
    session_no: Option<i64>,
    exclude_appointment: Option<&str>,
) -> Result<Option<(String, i64, String)>> {
    let Some(item_id) = item_id.filter(|s| !s.is_empty()) else { return Ok(None) };
    let item = get_item(conn, item_id).map_err(|e| e.rename_field("treatment_item_id"))?;
    if item.patient_id != patient_id {
        return Err(CoreError::invalid(
            "treatment_item_id",
            ValidationRule::ItemPatient,
            "that treatment is another patient's",
        ));
    }
    if !open(item.status) {
        return Err(CoreError::invalid(
            "treatment_item_id",
            ValidationRule::ItemNotEditable,
            "this treatment is finished",
        ));
    }
    let booked: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM appointment WHERE treatment_item_id = ?1 AND deleted_at IS NULL
             AND status IN {LIVE_STATUSES_SQL} AND id IS NOT ?2"
        ),
        params![item_id, exclude_appointment],
        |r| r.get(0),
    )?;
    let n = session_no.unwrap_or(item.sessions_done + booked + 1);
    if !(1..=item.sessions_planned).contains(&n) {
        return Err(CoreError::invalid(
            "session_no",
            ValidationRule::SessionNumber,
            format!("session {n} of {} — every session is done or booked", item.sessions_planned),
        ));
    }
    Ok(Some((item.id, n, item.service_id)))
}

/// A visit booked for a session was completed: its session is recorded (once).
pub fn record_for_appointment(conn: &Connection, actor: &Actor, appt: &AppointmentInfo) -> Result<()> {
    let Some(t) = &appt.treatment else { return Ok(()) };
    let item = get_item(conn, &t.item_id)?;
    let recorded: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM treatment_session WHERE item_id = ?1 AND appointment_id = ?2)",
        params![item.id, appt.id],
        |r| r.get(0),
    )?;
    let plan_open = match &item.plan_id {
        Some(pid) => plan_editable(get_plan(conn, pid)?.status),
        None => true,
    };
    if recorded || !open(item.status) || !plan_open {
        return Ok(());
    }
    insert_session(
        conn,
        actor,
        &item.id,
        Some(&appt.id),
        Some(&appt.doctor_id),
        Some(appt.date.clone()),
        None,
    )?;
    Ok(())
}

// ───────────────────────────── printed estimate (5.7) ─────────────────────────────

fn pick(t: &Translations, lang: Language) -> String {
    match lang {
        Language::Fa => t.fa.clone(),
        Language::Ps => t.ps.clone(),
        Language::En => t.en.clone(),
    }
}

/// "Root canal treatment — Two canals" in `lang`.
pub fn service_name(conn: &Connection, s: &ServiceInfo, lang: Language) -> Result<String> {
    Ok(match &s.parent_id {
        Some(pid) => {
            let parent = crate::catalog::get_service(conn, pid)?;
            format!("{} — {}", pick(&parent.name, lang), pick(&s.name, lang))
        }
        None => pick(&s.name, lang),
    })
}

/// A plan as printed on an estimate: its items (not cancelled) in the document's language, with totals.
pub fn plan_snapshot(
    conn: &Connection,
    patient_id: &str,
    plan_id: &str,
    lang: Language,
) -> Result<PlanSnapshot> {
    let plan = get_plan(conn, plan_id).map_err(|e| e.rename_field("plan_ids"))?;
    if plan.patient_id != patient_id {
        return Err(CoreError::invalid(
            "plan_ids",
            ValidationRule::ItemPatient,
            "that plan is another patient's",
        ));
    }
    let mut items = Vec::new();
    for i in plan.items.iter().filter(|i| i.status != ItemStatus::Cancelled) {
        let s = crate::catalog::get_service(conn, &i.service_id)?;
        items.push(PlanSnapshotItem {
            phase: i.phase,
            code: s.code.clone(),
            service: service_name(conn, &s, lang)?,
            teeth: i.teeth.clone(),
            surfaces: i.surfaces.clone(),
            sessions: i.sessions_planned,
            units: i.units,
            unit_price: i.unit_price,
            total: i.unit_price * i.units,
            status: i.status,
        });
    }
    Ok(PlanSnapshot { plan_id: plan.id, title: plan.title, total: plan.total, items })
}

// ───────────────────────────── ready-made plans ─────────────────────────────

#[derive(Deserialize)]
struct TemplateSeed {
    code: String,
    name: Translations,
    items: Vec<TemplateSeedItem>,
}

#[derive(Deserialize)]
struct TemplateSeedItem {
    service: String,
    teeth: TemplateTeeth,
    phase: i64,
}

fn template_seeds() -> Result<Vec<TemplateSeed>> {
    serde_json::from_str(PLAN_TEMPLATES_JSON)
        .map_err(|e| CoreError::validation(format!("plan_templates.json: {e}")))
}

/// Built-in ready-made plans are written once (then they are the clinic's to change).
pub fn sync_seeds(conn: &Connection) -> Result<()> {
    let now = now_iso();
    for (n, t) in template_seeds()?.iter().enumerate() {
        let items: Vec<PlanTemplateItem> = t
            .items
            .iter()
            .map(|i| PlanTemplateItem {
                service_id: seed_id("service", &i.service),
                teeth: i.teeth,
                phase: i.phase,
            })
            .collect();
        conn.execute(
            "INSERT INTO plan_template(id, code, name_fa, name_ps, name_en, items_json, sort_order, is_system, created_at,
                updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8)
             ON CONFLICT(id) DO NOTHING",
            params![
                seed_id("plan_template", &t.code),
                t.code,
                t.name.fa,
                t.name.ps,
                t.name.en,
                serde_json::to_string(&items)?,
                (n as i64 + 1) * 10,
                now
            ],
        )?;
    }
    Ok(())
}

const TEMPLATE_SELECT: &str =
    "SELECT id, code, name_fa, name_ps, name_en, items_json, is_system, is_active, version
    FROM plan_template WHERE deleted_at IS NULL";

fn map_template(r: &rusqlite::Row<'_>) -> rusqlite::Result<PlanTemplateInfo> {
    let items: String = r.get(5)?;
    Ok(PlanTemplateInfo {
        id: r.get(0)?,
        code: r.get(1)?,
        name: Translations { fa: r.get(2)?, ps: r.get(3)?, en: r.get(4)? },
        items: serde_json::from_str(&items).unwrap_or_default(),
        is_system: r.get(6)?,
        is_active: r.get(7)?,
        version: r.get(8)?,
    })
}

pub fn list_plan_templates(conn: &Connection, include_inactive: bool) -> Result<Vec<PlanTemplateInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{TEMPLATE_SELECT} AND (?1 = 1 OR is_active = 1) ORDER BY sort_order, created_at"
    ))?;
    let v = stmt.query_map([include_inactive], map_template)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn get_plan_template(conn: &Connection, id: &str) -> Result<PlanTemplateInfo> {
    conn.query_row(&format!("{TEMPLATE_SELECT} AND id = ?1"), [id], map_template).optional()?.ok_or_else(
        || {
            CoreError::api(ErrorCode::NotFound, format!("plan template {id}"))
                .on_field("id", ValidationRule::PlanTemplateNotFound)
        },
    )
}

pub fn save_plan_template(
    conn: &Connection,
    actor: &Actor,
    p: &SavePlanTemplateParams,
) -> Result<PlanTemplateInfo> {
    let name = clean_translations("name", &p.name, 120)?;
    if p.items.is_empty() || p.items.len() > 20 {
        return Err(CoreError::invalid(
            "items",
            ValidationRule::PlanTemplateItems,
            "a ready-made plan has 1 to 20 lines",
        ));
    }
    for (n, i) in p.items.iter().enumerate() {
        crate::catalog::get_service(conn, &i.service_id)
            .map_err(|e| e.rename_field(&format!("items.{n}.service_id")))?;
        if !(1..=5).contains(&i.phase) {
            return Err(CoreError::invalid(
                &format!("items.{n}.phase"),
                ValidationRule::ItemPhase,
                "phase 1 to 5",
            ));
        }
    }
    let items_json = serde_json::to_string(&p.items)?;
    let now = now_iso();
    let (id, before) = match &p.id {
        None => {
            let id = new_id();
            let sort: i64 =
                conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 10 FROM plan_template", [], |r| {
                    r.get(0)
                })?;
            conn.execute(
                "INSERT INTO plan_template(id, code, name_fa, name_ps, name_en, items_json, sort_order, is_active, created_at,
                    created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?9, ?10)",
                params![
                    id,
                    format!("custom_{}", id.replace('-', "")),
                    name.fa,
                    name.ps,
                    name.en,
                    items_json,
                    sort,
                    p.is_active,
                    now,
                    actor.user_id
                ],
            )?;
            (id, None)
        }
        Some(id) => {
            let before = get_plan_template(conn, id)?;
            let changed = conn.execute(
                "UPDATE plan_template SET name_fa=?1, name_ps=?2, name_en=?3, items_json=?4, is_active=?5, updated_at=?6,
                    updated_by=?7, version=version+1
                 WHERE id=?8 AND version=?9 AND deleted_at IS NULL",
                params![name.fa, name.ps, name.en, items_json, p.is_active, now, actor.user_id, id, p.version],
            )?;
            expect_one_row(changed, "plan template")?;
            (id.clone(), Some(before))
        }
    };
    let after = get_plan_template(conn, &id)?;
    audit::record(
        conn,
        actor,
        if before.is_some() { "plan_template.update" } else { "plan_template.create" },
        Some("plan_template"),
        Some(&id),
        before.as_ref().map(|b| json!(b)).as_ref(),
        Some(&json!(after)),
    )?;
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_follow_the_tooth_scope() {
        assert_eq!(units_for(ToothScope::Teeth, &[14, 15, 16]), 3);
        assert_eq!(units_for(ToothScope::Quadrant, &[14, 15, 24]), 2);
        assert_eq!(units_for(ToothScope::Arch, &[11, 21, 31]), 2);
        assert_eq!(units_for(ToothScope::Arch, &[]), 1);
        assert_eq!(units_for(ToothScope::Mouth, &[11, 21]), 1);
        assert_eq!(units_for(ToothScope::Tooth, &[36]), 1);
    }

    #[test]
    fn plan_template_seeds_point_at_catalog_codes() {
        let catalog: std::collections::HashSet<String> = {
            let v: serde_json::Value = serde_json::from_str(include_str!("../seeds/services.json")).unwrap();
            let mut codes = std::collections::HashSet::new();
            for c in v.as_array().unwrap() {
                for s in c["services"].as_array().unwrap() {
                    codes.insert(s["code"].as_str().unwrap().to_string());
                    for x in s["variants"].as_array().into_iter().flatten() {
                        codes.insert(x["code"].as_str().unwrap().to_string());
                    }
                }
            }
            codes
        };
        for t in template_seeds().unwrap() {
            assert!(!t.name.fa.contains(['ي', 'ك']), "{}", t.code);
            for i in &t.items {
                assert!(catalog.contains(&i.service), "{}: {}", t.code, i.service);
                assert!((1..=5).contains(&i.phase));
            }
        }
    }
}
