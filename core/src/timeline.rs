//! Patient timeline (5.11): every event of a patient's history, newest first — registration, visits,
//! examinations (signed notes), chart findings, plans, treatment sessions and completions,
//! prescriptions and documents, follow-ups, files, medical-history reviews and perio exams. It is read
//! from the records themselves (nothing extra is stored), so it can never disagree with them. The same
//! builder, narrowed to one tooth, is a tooth's history on the chart.

use artaveo_shared::{TimelineEvent, TimelineKind};
use rusqlite::Connection;

use crate::chart::parse_teeth;
use crate::clock::{parse, CLINIC_OFFSET};
use crate::error::Result;

/// A clinic-local date as an instant (11:00 in the clinic) for events that only have a date.
fn date_at(date: &str) -> String {
    format!("{date}T06:30:00.000Z")
}

/// When a dated event (a session, an exam) happened: the moment it was written down when that was on
/// the same day, else the day itself.
fn dated(date: &str, created_at: &str) -> String {
    let same_day = parse(created_at).map(|t| {
        let d = t.to_offset(CLINIC_OFFSET).date();
        format!("{:04}-{:02}-{:02}", d.year(), d.month() as u8, d.day()) == date
    });
    if same_day == Some(true) {
        created_at.to_string()
    } else {
        date_at(date)
    }
}

fn first_line(s: &str, max: usize) -> String {
    let line = s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    if line.chars().count() > max {
        format!("{}…", line.chars().take(max).collect::<String>())
    } else {
        line.to_string()
    }
}

/// A new event; the fields that apply are set on it before it is pushed.
fn ev(at: String, kind: TimelineKind, action: &str) -> TimelineEvent {
    TimelineEvent {
        at,
        kind,
        action: action.to_string(),
        ref_id: None,
        title: None,
        service_id: None,
        condition_id: None,
        teeth: Vec::new(),
        surfaces: None,
        session: None,
        detail: None,
        by_name: None,
    }
}

struct Builder<'a> {
    kinds: &'a [TimelineKind],
    tooth: Option<u8>,
    out: Vec<TimelineEvent>,
}

impl Builder<'_> {
    fn push(&mut self, mut e: TimelineEvent) {
        let kind_ok = self.kinds.is_empty() || self.kinds.contains(&e.kind);
        let tooth_ok = self.tooth.is_none_or(|t| e.teeth.contains(&t));
        if !kind_ok || !tooth_ok {
            return;
        }
        e.surfaces = e.surfaces.filter(|s| !s.is_empty());
        e.detail = e.detail.filter(|s| !s.is_empty());
        self.out.push(e);
    }
}

/// The events of one patient (optionally of one tooth only), newest first, at most `limit`.
pub fn events(
    conn: &Connection,
    patient_id: &str,
    kinds: &[TimelineKind],
    limit: u32,
    tooth: Option<u8>,
) -> Result<Vec<TimelineEvent>> {
    use TimelineKind::*;
    let mut b = Builder { kinds, tooth, out: Vec::new() };
    let whole = tooth.is_none();

    if whole {
        let (created, by): (String, Option<String>) = conn.query_row(
            "SELECT p.created_at, u.display_name FROM patient p LEFT JOIN app_user u ON u.id = p.created_by WHERE p.id = ?1",
            [patient_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        b.push(TimelineEvent { by_name: by, ..ev(created, Registered, "patient.registered") });

        let mut stmt = conn.prepare(
            "SELECT a.id, a.start_at, a.status, d.full_name, a.service_id, a.reason, a.session_no, ti.teeth,
                ti.sessions_planned
             FROM appointment a JOIN doctor d ON d.id = a.doctor_id
             LEFT JOIN treatment_item ti ON ti.id = a.treatment_item_id
             WHERE a.patient_id = ?1 AND a.deleted_at IS NULL",
        )?;
        let mut rows = stmt.query([patient_id])?;
        while let Some(r) = rows.next()? {
            let status: String = r.get(2)?;
            let session = r.get::<_, Option<i64>>(6)?.zip(r.get::<_, Option<i64>>(8)?);
            b.push(TimelineEvent {
                ref_id: Some(r.get(0)?),
                title: Some(r.get(3)?),
                service_id: r.get(4)?,
                detail: r.get(5)?,
                teeth: r.get::<_, Option<String>>(7)?.as_deref().map(parse_teeth).unwrap_or_default(),
                session: session.map(|(n, of)| format!("{n}/{of}")),
                ..ev(r.get(1)?, Appointment, &format!("appointment.{status}"))
            });
        }

        let mut stmt = conn.prepare(
            "SELECT n.id, n.signed_at, d.full_name, n.diagnosis, n.chief_complaint, n.procedure, u.display_name
             FROM clinical_note n LEFT JOIN doctor d ON d.id = n.doctor_id LEFT JOIN app_user u ON u.id = n.signed_by
             WHERE n.patient_id = ?1 AND n.deleted_at IS NULL AND n.status = 'signed'",
        )?;
        let mut rows = stmt.query([patient_id])?;
        while let Some(r) = rows.next()? {
            let parts: [String; 3] = [r.get(3)?, r.get(4)?, r.get(5)?];
            let detail = parts.into_iter().find(|s| !s.trim().is_empty()).map(|d| first_line(&d, 140));
            b.push(TimelineEvent {
                ref_id: Some(r.get(0)?),
                title: r.get(2)?,
                detail,
                by_name: r.get(6)?,
                ..ev(r.get(1)?, Examination, "note.signed")
            });
        }
    }

    // Findings: added, and removed (kept with who removed them).
    let mut stmt = conn.prepare(
        "SELECT f.id, f.created_at, f.condition_id, f.tooth, f.surfaces, f.note, u.display_name, f.deleted_at,
            x.display_name
         FROM tooth_finding f LEFT JOIN app_user u ON u.id = f.created_by LEFT JOIN app_user x ON x.id = f.deleted_by
         WHERE f.patient_id = ?1",
    )?;
    let mut rows = stmt.query([patient_id])?;
    while let Some(r) = rows.next()? {
        let base = TimelineEvent {
            ref_id: Some(r.get(0)?),
            condition_id: Some(r.get(2)?),
            teeth: vec![r.get(3)?],
            surfaces: Some(r.get(4)?),
            ..ev(String::new(), Diagnosis, "")
        };
        b.push(TimelineEvent {
            at: r.get(1)?,
            action: "finding.added".into(),
            detail: r.get(5)?,
            by_name: r.get(6)?,
            ..base.clone()
        });
        if let Some(removed) = r.get::<_, Option<String>>(7)? {
            b.push(TimelineEvent {
                at: removed,
                action: "finding.removed".into(),
                by_name: r.get(8)?,
                ..base
            });
        }
    }

    if whole {
        let mut stmt = conn.prepare(
            "SELECT p.id, p.title, p.created_at, p.accepted_at, p.completed_at, p.cancelled_at, p.superseded_by,
                u.display_name
             FROM treatment_plan p LEFT JOIN app_user u ON u.id = p.created_by
             WHERE p.patient_id = ?1 AND p.deleted_at IS NULL",
        )?;
        let mut rows = stmt.query([patient_id])?;
        while let Some(r) = rows.next()? {
            let plan = TimelineEvent {
                ref_id: Some(r.get(0)?),
                title: Some(r.get(1)?),
                ..ev(String::new(), Plan, "")
            };
            b.push(TimelineEvent {
                at: r.get(2)?,
                action: "plan.created".into(),
                by_name: r.get(7)?,
                ..plan.clone()
            });
            for (col, action) in [(3, "plan.accepted"), (4, "plan.completed")] {
                if let Some(at) = r.get::<_, Option<String>>(col)? {
                    b.push(TimelineEvent { at, action: action.into(), ..plan.clone() });
                }
            }
            if let Some(at) = r.get::<_, Option<String>>(5)? {
                let superseded = r.get::<_, Option<String>>(6)?.is_some();
                let action = if superseded { "plan.superseded" } else { "plan.cancelled" };
                b.push(TimelineEvent { at, action: action.into(), ..plan });
            }
        }
    }

    // Treatment items: planned and cancelled (on a tooth's history), completed (everywhere).
    let mut stmt = conn.prepare(
        "SELECT i.id, i.service_id, i.teeth, i.surfaces, i.created_at, i.completed_at, i.cancelled_at, i.cancel_reason,
            u.display_name
         FROM treatment_item i LEFT JOIN app_user u ON u.id = i.created_by
         WHERE i.patient_id = ?1 AND i.deleted_at IS NULL",
    )?;
    let mut rows = stmt.query([patient_id])?;
    while let Some(r) = rows.next()? {
        let item = TimelineEvent {
            ref_id: Some(r.get(0)?),
            service_id: Some(r.get(1)?),
            teeth: parse_teeth(&r.get::<_, String>(2)?),
            surfaces: Some(r.get(3)?),
            ..ev(String::new(), Plan, "")
        };
        if !whole {
            b.push(TimelineEvent {
                at: r.get(4)?,
                action: "item.planned".into(),
                by_name: r.get(8)?,
                ..item.clone()
            });
            if let Some(at) = r.get::<_, Option<String>>(6)? {
                b.push(TimelineEvent {
                    at,
                    action: "item.cancelled".into(),
                    detail: r.get(7)?,
                    ..item.clone()
                });
            }
        }
        if let Some(at) = r.get::<_, Option<String>>(5)? {
            b.push(TimelineEvent { at, kind: Treatment, action: "item.completed".into(), ..item });
        }
    }

    let mut stmt = conn.prepare(
        "SELECT s.id, s.performed_on, s.created_at, s.session_no, i.sessions_planned, i.service_id, i.teeth, i.surfaces,
            s.note, d.full_name
         FROM treatment_session s JOIN treatment_item i ON i.id = s.item_id LEFT JOIN doctor d ON d.id = s.doctor_id
         WHERE s.patient_id = ?1",
    )?;
    let mut rows = stmt.query([patient_id])?;
    while let Some(r) = rows.next()? {
        let on: String = r.get(1)?;
        let created: String = r.get(2)?;
        b.push(TimelineEvent {
            ref_id: Some(r.get(0)?),
            session: Some(format!("{}/{}", r.get::<_, i64>(3)?, r.get::<_, i64>(4)?)),
            service_id: Some(r.get(5)?),
            teeth: parse_teeth(&r.get::<_, String>(6)?),
            surfaces: Some(r.get(7)?),
            detail: r.get(8)?,
            by_name: r.get(9)?,
            ..ev(dated(&on, &created), Treatment, "session.recorded")
        });
    }

    if whole {
        let mut stmt = conn.prepare(
            "SELECT d.id, d.kind, d.number, d.issued_at, d.status, d.updated_at, u.display_name
             FROM clinical_document d LEFT JOIN app_user u ON u.id = d.created_by
             WHERE d.patient_id = ?1 AND d.deleted_at IS NULL",
        )?;
        let mut rows = stmt.query([patient_id])?;
        while let Some(r) = rows.next()? {
            let kind: String = r.get(1)?;
            let k = if kind == "prescription" { Prescription } else { Document };
            let doc = TimelineEvent {
                ref_id: Some(r.get(0)?),
                title: Some(r.get(2)?),
                detail: Some(kind),
                ..ev(String::new(), k, "")
            };
            b.push(TimelineEvent {
                at: r.get(3)?,
                action: "document.issued".into(),
                by_name: r.get(6)?,
                ..doc.clone()
            });
            if r.get::<_, String>(4)? == "void" {
                b.push(TimelineEvent { at: r.get(5)?, action: "document.void".into(), ..doc });
            }
        }

        let mut stmt = conn.prepare(
            "SELECT r.id, r.created_at, r.kind, r.due_date, r.status, u.display_name FROM recall r
             LEFT JOIN app_user u ON u.id = r.created_by WHERE r.patient_id = ?1 AND r.deleted_at IS NULL",
        )?;
        let mut rows = stmt.query([patient_id])?;
        while let Some(r) = rows.next()? {
            let status: String = r.get(4)?;
            b.push(TimelineEvent {
                ref_id: Some(r.get(0)?),
                title: Some(r.get(2)?),
                detail: Some(r.get(3)?),
                by_name: r.get(5)?,
                ..ev(r.get(1)?, FollowUp, &format!("recall.{status}"))
            });
        }

        let mut stmt = conn.prepare(
            "SELECT a.id, a.created_at, a.file_name, a.kind, a.tooth, u.display_name FROM patient_attachment a
             LEFT JOIN app_user u ON u.id = a.created_by WHERE a.patient_id = ?1 AND a.deleted_at IS NULL",
        )?;
        let mut rows = stmt.query([patient_id])?;
        while let Some(r) = rows.next()? {
            b.push(TimelineEvent {
                ref_id: Some(r.get(0)?),
                title: Some(r.get(2)?),
                detail: Some(r.get(3)?),
                teeth: r.get::<_, Option<String>>(4)?.as_deref().map(parse_teeth).unwrap_or_default(),
                by_name: r.get(5)?,
                ..ev(r.get(1)?, Attachment, "attachment.added")
            });
        }

        let reviewed: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT h.reviewed_at, u.display_name FROM patient_medical_history h
                 LEFT JOIN app_user u ON u.id = h.reviewed_by WHERE h.patient_id = ?1 AND h.deleted_at IS NULL",
                [patient_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        if let Some((Some(at), by)) = reviewed {
            b.push(TimelineEvent { by_name: by, ..ev(at, MedicalHistory, "medical.reviewed") });
        }

        for e in crate::perio::list(conn, patient_id)? {
            let mean = format!("{}.{}", e.summary.mean_depth_x10 / 10, e.summary.mean_depth_x10 % 10);
            b.push(TimelineEvent {
                ref_id: Some(e.id),
                title: e.doctor_name,
                detail: Some(mean),
                ..ev(dated(&e.exam_date, &e.created_at), Perio, "perio.recorded")
            });
        }
    }

    let mut out = b.out;
    out.sort_by(|a, b| b.at.cmp(&a.at));
    out.truncate(limit.clamp(1, 2000) as usize);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dated_event_takes_its_moment_only_on_the_same_day() {
        // 2026-10-11 08:00 UTC is 12:30 in Kabul on the 11th.
        assert_eq!(dated("2026-10-11", "2026-10-11T08:00:00.000Z"), "2026-10-11T08:00:00.000Z");
        assert_eq!(dated("2026-10-09", "2026-10-11T08:00:00.000Z"), "2026-10-09T06:30:00.000Z");
        // 21:00 UTC on the 10th is already the 11th in Kabul.
        assert_eq!(dated("2026-10-11", "2026-10-10T21:00:00.000Z"), "2026-10-10T21:00:00.000Z");
        assert_eq!(first_line("\n  first \nsecond", 10), "first");
        assert_eq!(first_line("abcdefghijk", 5), "abcde…");
    }
}
