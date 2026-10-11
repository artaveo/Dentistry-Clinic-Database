//! Periodontal chart (5.5): per tooth six probing sites (distobuccal, buccal, mesiobuccal, distolingual,
//! lingual, mesiolingual) with pocket depth, recession, bleeding and plaque, plus mobility and furcation.
//! Each exam is kept whole so two exams can be compared; the summary (mean depth, deep sites,
//! bleeding %) is computed by the Core.

use artaveo_shared::{ErrorCode, PerioExamInfo, PerioSummary, PerioTooth, SavePerioParams, ValidationRule};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::clock::{now_iso, today_iso};
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::new_id;
use crate::scheduling::{format_date, parse_date};

fn bad(field: String, detail: &str) -> CoreError {
    CoreError::invalid(&field, ValidationRule::PerioValue, detail.to_string())
}

/// Checks one exam's teeth: FDI numbers once each, six values per site row, values in range.
fn check_teeth(teeth: &[PerioTooth]) -> Result<Vec<PerioTooth>> {
    let mut seen = Vec::new();
    let mut out = Vec::with_capacity(teeth.len());
    for (n, t) in teeth.iter().enumerate() {
        let f = |name: &str| format!("teeth.{n}.{name}");
        if !crate::chart::is_tooth(t.tooth) {
            return Err(CoreError::invalid(
                &f("tooth"),
                ValidationRule::ToothNumber,
                "not an FDI tooth number",
            ));
        }
        if seen.contains(&t.tooth) {
            return Err(bad(f("tooth"), "a tooth appears twice"));
        }
        seen.push(t.tooth);
        if t.depth.len() != 6 || t.recession.len() != 6 || t.bleeding.len() != 6 || t.plaque.len() != 6 {
            return Err(bad(f("depth"), "six sites per tooth"));
        }
        if t.furcation.len() != 2 {
            return Err(bad(f("furcation"), "furcation at the buccal and the lingual"));
        }
        if t.depth.iter().flatten().any(|&d| d > 15) {
            return Err(bad(f("depth"), "pocket depth is 0 to 15 mm"));
        }
        if t.recession.iter().flatten().any(|&r| !(-5..=15).contains(&r)) {
            return Err(bad(f("recession"), "recession is -5 to 15 mm"));
        }
        if t.mobility.is_some_and(|m| m > 3) {
            return Err(bad(f("mobility"), "mobility is 0 to 3"));
        }
        if t.furcation.iter().flatten().any(|&g| g > 3) {
            return Err(bad(f("furcation"), "furcation grade is 0 to 3"));
        }
        // A missing tooth keeps no measurements.
        out.push(if t.missing {
            PerioTooth {
                tooth: t.tooth,
                missing: true,
                implant: false,
                depth: vec![None; 6],
                recession: vec![None; 6],
                bleeding: vec![false; 6],
                plaque: vec![false; 6],
                mobility: None,
                furcation: vec![None; 2],
            }
        } else {
            t.clone()
        });
    }
    out.sort_by_key(|t| t.tooth);
    Ok(out)
}

pub fn summary(teeth: &[PerioTooth]) -> PerioSummary {
    let present: Vec<&PerioTooth> = teeth.iter().filter(|t| !t.missing).collect();
    let depths: Vec<u32> = present.iter().flat_map(|t| t.depth.iter().flatten().map(|&d| d as u32)).collect();
    let sites = depths.len() as u32;
    let all_sites = (present.len() * 6) as u32;
    let pct = |n: usize| (n as u32 * 100).checked_div(all_sites).unwrap_or(0);
    PerioSummary {
        teeth: present.len() as u32,
        sites,
        mean_depth_x10: (depths.iter().sum::<u32>() * 10 + sites / 2).checked_div(sites).unwrap_or(0),
        sites_4_plus: depths.iter().filter(|&&d| d >= 4).count() as u32,
        sites_6_plus: depths.iter().filter(|&&d| d >= 6).count() as u32,
        bleeding_percent: pct(present.iter().map(|t| t.bleeding.iter().filter(|b| **b).count()).sum()),
        plaque_percent: pct(present.iter().map(|t| t.plaque.iter().filter(|b| **b).count()).sum()),
    }
}

const SELECT: &str =
    "SELECT e.id, e.patient_id, e.exam_date, e.doctor_id, d.full_name, e.data_json, e.notes, e.created_at,
    e.version FROM perio_exam e LEFT JOIN doctor d ON d.id = e.doctor_id WHERE e.deleted_at IS NULL";

fn map(r: &rusqlite::Row<'_>) -> rusqlite::Result<PerioExamInfo> {
    let data: String = r.get(5)?;
    let teeth: Vec<PerioTooth> = serde_json::from_str(&data).unwrap_or_default();
    Ok(PerioExamInfo {
        id: r.get(0)?,
        patient_id: r.get(1)?,
        exam_date: r.get(2)?,
        doctor_id: r.get(3)?,
        doctor_name: r.get(4)?,
        summary: summary(&teeth),
        teeth,
        notes: r.get(6)?,
        created_at: r.get(7)?,
        version: r.get(8)?,
    })
}

pub fn get(conn: &Connection, id: &str) -> Result<PerioExamInfo> {
    conn.query_row(&format!("{SELECT} AND e.id = ?1"), [id], map).optional()?.ok_or_else(|| {
        CoreError::api(ErrorCode::NotFound, format!("perio exam {id}"))
            .on_field("id", ValidationRule::PerioExamNotFound)
    })
}

/// A patient's exams, newest first.
pub fn list(conn: &Connection, patient_id: &str) -> Result<Vec<PerioExamInfo>> {
    let mut stmt = conn
        .prepare(&format!("{SELECT} AND e.patient_id = ?1 ORDER BY e.exam_date DESC, e.created_at DESC"))?;
    let v = stmt.query_map([patient_id], map)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn save(conn: &Connection, actor: &Actor, p: &SavePerioParams) -> Result<PerioExamInfo> {
    crate::treatment::check_patient(conn, &p.patient_id)?;
    let teeth = check_teeth(&p.teeth)?;
    if teeth.is_empty() {
        return Err(bad("teeth".into(), "an exam needs at least one tooth"));
    }
    let today = today_iso();
    let exam_date = match p.exam_date.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => format_date(parse_date("exam_date", d)?),
        None => today.clone(),
    };
    if exam_date > today {
        return Err(CoreError::invalid(
            "exam_date",
            ValidationRule::DateFormat,
            "an exam cannot be in the future",
        ));
    }
    let doctor = match p.doctor_id.as_deref().filter(|d| !d.is_empty()) {
        Some(d) => Some(crate::scheduling::get_doctor(conn, d).map_err(|e| e.rename_field("doctor_id"))?.id),
        None => None,
    };
    let notes = p.notes.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
    if notes.as_deref().is_some_and(|n| n.chars().count() > 2000) {
        return Err(CoreError::invalid(
            "notes",
            ValidationRule::DocumentText,
            "notes are at most 2000 characters",
        ));
    }
    let data = serde_json::to_string(&teeth)?;
    let now = now_iso();
    let (id, before) = match &p.id {
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO perio_exam(id, patient_id, exam_date, doctor_id, data_json, notes, created_at, created_by,
                    updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?7, ?8)",
                params![id, p.patient_id, exam_date, doctor, data, notes, now, actor.user_id],
            )?;
            (id, None)
        }
        Some(id) => {
            let before = get(conn, id).map_err(|e| e.rename_field("id"))?;
            if before.patient_id != p.patient_id {
                return Err(CoreError::invalid(
                    "patient_id",
                    ValidationRule::ItemPatient,
                    "the exam is another patient's",
                ));
            }
            let changed = conn.execute(
                "UPDATE perio_exam SET exam_date=?1, doctor_id=?2, data_json=?3, notes=?4, updated_at=?5, updated_by=?6,
                    version=version+1
                 WHERE id=?7 AND version=?8 AND deleted_at IS NULL",
                params![exam_date, doctor, data, notes, now, actor.user_id, id, p.version],
            )?;
            expect_one_row(changed, "perio exam")?;
            (id.clone(), Some(before))
        }
    };
    let after = get(conn, &id)?;
    audit::record(
        conn,
        actor,
        if before.is_some() { "perio.update" } else { "perio.create" },
        Some("patient"),
        Some(&p.patient_id),
        before.as_ref().map(|b| json!({"exam_id": b.id, "summary": b.summary})).as_ref(),
        Some(&json!({"exam_id": after.id, "exam_date": after.exam_date, "summary": after.summary})),
    )?;
    Ok(after)
}

pub fn delete(conn: &Connection, actor: &Actor, id: &str, version: i64) -> Result<()> {
    let before = get(conn, id).map_err(|e| e.rename_field("id"))?;
    let now = now_iso();
    let changed = conn.execute(
        "UPDATE perio_exam SET deleted_at=?1, updated_at=?1, updated_by=?2, version=version+1
         WHERE id=?3 AND version=?4 AND deleted_at IS NULL",
        params![now, actor.user_id, id, version],
    )?;
    expect_one_row(changed, "perio exam")?;
    audit::record(
        conn,
        actor,
        "perio.delete",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!({"exam_id": id, "exam_date": before.exam_date})),
        None,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tooth(t: u8, depth: [u8; 6], bleeding: usize) -> PerioTooth {
        PerioTooth {
            tooth: t,
            missing: false,
            implant: false,
            depth: depth.iter().map(|&d| Some(d)).collect(),
            recession: vec![Some(0); 6],
            bleeding: (0..6).map(|i| i < bleeding).collect(),
            plaque: vec![false; 6],
            mobility: Some(0),
            furcation: vec![None; 2],
        }
    }

    #[test]
    fn summary_counts_depths_and_bleeding() {
        let mut gone = tooth(18, [9; 6], 6);
        gone.missing = true;
        let s = summary(&[tooth(16, [3, 2, 4, 3, 2, 6], 3), tooth(26, [2; 6], 0), gone]);
        assert_eq!((s.teeth, s.sites), (2, 12));
        assert_eq!(s.mean_depth_x10, 27); // 32 mm / 12 sites = 2.67
        assert_eq!((s.sites_4_plus, s.sites_6_plus), (2, 1));
        assert_eq!(s.bleeding_percent, 25);
    }

    #[test]
    fn exams_are_checked() {
        assert!(check_teeth(&[tooth(16, [3; 6], 0)]).is_ok());
        assert!(check_teeth(&[tooth(19, [3; 6], 0)]).is_err());
        assert!(check_teeth(&[tooth(16, [16, 0, 0, 0, 0, 0], 0)]).is_err());
        assert!(check_teeth(&[tooth(16, [3; 6], 0), tooth(16, [3; 6], 0)]).is_err());
        let mut short = tooth(16, [3; 6], 0);
        short.depth.pop();
        assert!(check_teeth(&[short]).is_err());
    }
}
