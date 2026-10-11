//! Dental chart (5.2–5.4 and M5 of docs/specs/clinical-workflow-v1.md): which teeth a patient's chart
//! shows (permanent, primary or mixed), the tooth conditions the clinic records (seeded from
//! `core/seeds/tooth_conditions.json`, then the clinic's to extend), the findings of an examination
//! (the chart's "existing" layer) and each tooth's history. Planned and completed work lives in
//! [`crate::treatment`]; the chart reads both. Tooth numbers are always FDI (ISO 3950, ADR-14).

use artaveo_shared::{
    AddFindingsParams, ChartInfo, ChartSymbol, Dentition, ErrorCode, PlanBrief, SaveToothConditionParams,
    SetDentitionParams, TimelineEvent, ToothConditionInfo, ToothFinding, Translations, ValidationRule,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::json;

use crate::audit::{self, Actor};
use crate::clock::{now_iso, today_iso};
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::formulary::clean_translations;
use crate::ids::{new_id, seed_id};

const CONDITIONS_JSON: &str = include_str!("../seeds/tooth_conditions.json");

/// Places of a mixed dentition that show the baby tooth unless the doctor says otherwise: canines and
/// molars of the primary set next to the adult incisors and first molars (a child of about 8).
pub const DEFAULT_PRIMARY_SLOTS: [u8; 12] = [13, 14, 15, 23, 24, 25, 33, 34, 35, 43, 44, 45];

pub fn symbol_code(s: ChartSymbol) -> &'static str {
    match s {
        ChartSymbol::None => "none",
        ChartSymbol::Surface => "surface",
        ChartSymbol::Sealant => "sealant",
        ChartSymbol::Veneer => "veneer",
        ChartSymbol::Crown => "crown",
        ChartSymbol::RootCanal => "root_canal",
        ChartSymbol::Pulpotomy => "pulpotomy",
        ChartSymbol::Extraction => "extraction",
        ChartSymbol::Missing => "missing",
        ChartSymbol::Implant => "implant",
        ChartSymbol::Abutment => "abutment",
        ChartSymbol::ImplantCrown => "implant_crown",
        ChartSymbol::Bridge => "bridge",
        ChartSymbol::Denture => "denture",
        ChartSymbol::Bracket => "bracket",
        ChartSymbol::Fracture => "fracture",
        ChartSymbol::Impacted => "impacted",
        ChartSymbol::Lesion => "lesion",
        ChartSymbol::Scaling => "scaling",
        ChartSymbol::Marker => "marker",
    }
}

pub fn symbol_from(s: &str) -> ChartSymbol {
    match s {
        "surface" => ChartSymbol::Surface,
        "sealant" => ChartSymbol::Sealant,
        "veneer" => ChartSymbol::Veneer,
        "crown" => ChartSymbol::Crown,
        "root_canal" => ChartSymbol::RootCanal,
        "pulpotomy" => ChartSymbol::Pulpotomy,
        "extraction" => ChartSymbol::Extraction,
        "missing" => ChartSymbol::Missing,
        "implant" => ChartSymbol::Implant,
        "abutment" => ChartSymbol::Abutment,
        "implant_crown" => ChartSymbol::ImplantCrown,
        "bridge" => ChartSymbol::Bridge,
        "denture" => ChartSymbol::Denture,
        "bracket" => ChartSymbol::Bracket,
        "fracture" => ChartSymbol::Fracture,
        "impacted" => ChartSymbol::Impacted,
        "lesion" => ChartSymbol::Lesion,
        "scaling" => ChartSymbol::Scaling,
        "marker" => ChartSymbol::Marker,
        _ => ChartSymbol::None,
    }
}

fn dentition_code(d: Dentition) -> &'static str {
    match d {
        Dentition::Permanent => "permanent",
        Dentition::Primary => "primary",
        Dentition::Mixed => "mixed",
    }
}

fn dentition_from(s: &str) -> Dentition {
    match s {
        "primary" => Dentition::Primary,
        "mixed" => Dentition::Mixed,
        _ => Dentition::Permanent,
    }
}

// ───────────────────────────── tooth numbers and surfaces ─────────────────────────────

/// A valid FDI number: adult 11–18 … 41–48, baby 51–55 … 81–85.
pub fn is_tooth(t: u8) -> bool {
    let (q, n) = (t / 10, t % 10);
    matches!((q, n), (1..=4, 1..=8) | (5..=8, 1..=5))
}

pub fn is_primary(t: u8) -> bool {
    (51..=85).contains(&t)
}

/// 1 = upper right … 4 = lower right (a baby tooth's quadrant is that of its place).
pub fn quadrant(t: u8) -> u8 {
    match t / 10 {
        5 => 1,
        6 => 2,
        7 => 3,
        8 => 4,
        q => q,
    }
}

pub fn is_upper(t: u8) -> bool {
    matches!(quadrant(t), 1 | 2)
}

/// Checks and orders a list of teeth (no repeats).
pub fn check_teeth(field: &str, teeth: &[u8]) -> Result<Vec<u8>> {
    let mut v = Vec::with_capacity(teeth.len());
    for &t in teeth {
        if !is_tooth(t) {
            return Err(CoreError::invalid(
                field,
                ValidationRule::ToothNumber,
                format!("{t} is not an FDI tooth number"),
            ));
        }
        if !v.contains(&t) {
            v.push(t);
        }
    }
    v.sort_unstable();
    Ok(v)
}

/// Surfaces as `MODBL` letters in that order. Accepts the anterior/arch names too: I (incisal) = O,
/// F (facial/labial) = B, P (palatal) = L.
pub fn clean_surfaces(field: &str, s: &str) -> Result<String> {
    let mut have = [false; 5];
    for c in s.chars().filter(|c| !c.is_whitespace() && *c != ',') {
        let i = match c.to_ascii_uppercase() {
            'M' => 0,
            'O' | 'I' => 1,
            'D' => 2,
            'B' | 'F' => 3,
            'L' | 'P' => 4,
            _ => {
                return Err(CoreError::invalid(
                    field,
                    ValidationRule::ToothSurface,
                    format!("{c} is not a tooth surface"),
                ))
            }
        };
        have[i] = true;
    }
    Ok("MODBL".chars().zip(have).filter(|(_, h)| *h).map(|(c, _)| c).collect())
}

pub fn teeth_text(teeth: &[u8]) -> String {
    teeth.iter().map(u8::to_string).collect::<Vec<_>>().join(",")
}

pub fn parse_teeth(s: &str) -> Vec<u8> {
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

/// Every tooth of a dentition (mixed: each place shows the baby or the adult tooth).
pub fn all_teeth(dentition: Dentition, primary_slots: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    for q in 1..=4u8 {
        match dentition {
            Dentition::Permanent => v.extend((1..=8).map(|n| q * 10 + n)),
            Dentition::Primary => v.extend((1..=5).map(|n| (q + 4) * 10 + n)),
            Dentition::Mixed => {
                for n in 1..=8u8 {
                    let adult = q * 10 + n;
                    v.push(if n <= 5 && primary_slots.contains(&adult) { (q + 4) * 10 + n } else { adult });
                }
            }
        }
    }
    v
}

// ───────────────────────────── tooth conditions (5.3) ─────────────────────────────

#[derive(Deserialize)]
struct ConditionSeed {
    code: String,
    symbol: ChartSymbol,
    #[serde(default)]
    surface: bool,
    #[serde(default)]
    abbreviation: Option<String>,
    name: Translations,
}

fn condition_seeds() -> Result<Vec<ConditionSeed>> {
    serde_json::from_str(CONDITIONS_JSON)
        .map_err(|e| CoreError::validation(format!("tooth_conditions.json: {e}")))
}

/// Built-in conditions are written once; afterwards they are the clinic's (names, switched off).
pub fn sync_seeds(conn: &Connection) -> Result<()> {
    let now = now_iso();
    for (i, c) in condition_seeds()?.iter().enumerate() {
        conn.execute(
            "INSERT INTO tooth_condition(id, code, name_fa, name_ps, name_en, symbol, needs_surface, abbreviation, sort_order,
                is_system, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?10)
             ON CONFLICT(id) DO NOTHING",
            params![
                seed_id("tooth_condition", &c.code),
                c.code,
                c.name.fa,
                c.name.ps,
                c.name.en,
                symbol_code(c.symbol),
                c.surface,
                c.abbreviation,
                (i as i64 + 1) * 10,
                now
            ],
        )?;
    }
    Ok(())
}

const CONDITION_SELECT: &str =
    "SELECT id, code, name_fa, name_ps, name_en, symbol, needs_surface, abbreviation,
    is_system, is_active, sort_order, version FROM tooth_condition WHERE deleted_at IS NULL";

fn map_condition(r: &rusqlite::Row<'_>) -> rusqlite::Result<ToothConditionInfo> {
    Ok(ToothConditionInfo {
        id: r.get(0)?,
        code: r.get(1)?,
        name: Translations { fa: r.get(2)?, ps: r.get(3)?, en: r.get(4)? },
        symbol: symbol_from(&r.get::<_, String>(5)?),
        needs_surface: r.get(6)?,
        abbreviation: r.get(7)?,
        is_system: r.get(8)?,
        is_active: r.get(9)?,
        sort_order: r.get(10)?,
        version: r.get(11)?,
    })
}

pub fn list_conditions(conn: &Connection, include_inactive: bool) -> Result<Vec<ToothConditionInfo>> {
    let mut stmt =
        conn.prepare(&format!("{CONDITION_SELECT} AND (?1 = 1 OR is_active = 1) ORDER BY sort_order, code"))?;
    let v = stmt.query_map([include_inactive], map_condition)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn get_condition(conn: &Connection, id: &str) -> Result<ToothConditionInfo> {
    conn.query_row(&format!("{CONDITION_SELECT} AND id = ?1"), [id], map_condition).optional()?.ok_or_else(
        || {
            CoreError::api(ErrorCode::NotFound, format!("tooth condition {id}"))
                .on_field("condition_id", ValidationRule::ConditionNotFound)
        },
    )
}

pub fn save_condition(
    conn: &Connection,
    actor: &Actor,
    p: &SaveToothConditionParams,
) -> Result<ToothConditionInfo> {
    let name = clean_translations("name", &p.name, 80)?;
    let abbreviation = p.abbreviation.as_deref().map(str::trim).filter(|a| !a.is_empty()).map(str::to_string);
    if abbreviation.as_deref().is_some_and(|a| a.chars().count() > 3) {
        return Err(CoreError::invalid(
            "abbreviation",
            ValidationRule::ConditionAbbreviation,
            "an abbreviation is 1 to 3 letters",
        ));
    }
    let now = now_iso();
    let (id, before) = match &p.id {
        None => {
            let id = new_id();
            let sort: i64 =
                conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 10 FROM tooth_condition", [], |r| {
                    r.get(0)
                })?;
            conn.execute(
                "INSERT INTO tooth_condition(id, code, name_fa, name_ps, name_en, symbol, needs_surface, abbreviation,
                    sort_order, is_active, created_at, created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?11, ?12)",
                params![
                    id,
                    format!("custom_{}", id.replace('-', "")),
                    name.fa,
                    name.ps,
                    name.en,
                    symbol_code(p.symbol),
                    p.needs_surface,
                    abbreviation,
                    sort,
                    p.is_active,
                    now,
                    actor.user_id
                ],
            )?;
            (id, None)
        }
        Some(id) => {
            let before = get_condition(conn, id).map_err(|e| e.rename_field("id"))?;
            let changed = conn.execute(
                "UPDATE tooth_condition SET name_fa=?1, name_ps=?2, name_en=?3, symbol=?4, needs_surface=?5, abbreviation=?6,
                    is_active=?7, updated_at=?8, updated_by=?9, version=version+1
                 WHERE id=?10 AND version=?11 AND deleted_at IS NULL",
                params![
                    name.fa,
                    name.ps,
                    name.en,
                    symbol_code(p.symbol),
                    p.needs_surface,
                    abbreviation,
                    p.is_active,
                    now,
                    actor.user_id,
                    id,
                    p.version
                ],
            )?;
            expect_one_row(changed, "tooth condition")?;
            (id.clone(), Some(before))
        }
    };
    let after = get_condition(conn, &id)?;
    audit::record(
        conn,
        actor,
        if before.is_some() { "tooth_condition.update" } else { "tooth_condition.create" },
        Some("tooth_condition"),
        Some(&id),
        before.as_ref().map(|b| json!(b)).as_ref(),
        Some(&json!(after)),
    )?;
    Ok(after)
}

// ───────────────────────────── the chart ─────────────────────────────

/// The teeth a child's chart starts with: baby teeth under 6, mixed from 6 to 12, adult after.
fn dentition_for_age(age: Option<i64>) -> Dentition {
    match age {
        Some(a) if a < 6 => Dentition::Primary,
        Some(a) if a <= 12 => Dentition::Mixed,
        _ => Dentition::Permanent,
    }
}

fn patient_age(conn: &Connection, patient_id: &str) -> Result<Option<i64>> {
    let p = crate::patient::get_patient(conn, patient_id)?;
    let today = crate::clock::local_now().date();
    Ok(
        match p.date_of_birth.as_deref().and_then(|d| crate::scheduling::parse_date("date_of_birth", d).ok())
        {
            Some(dob) => {
                let mut age = today.year() as i64 - dob.year() as i64;
                if (today.month() as u8, today.day()) < (dob.month() as u8, dob.day()) {
                    age -= 1;
                }
                Some(age)
            }
            None => p.approximate_age,
        },
    )
}

/// (dentition, primary slots, chosen, version) of a patient's chart.
pub fn dentition_of(conn: &Connection, patient_id: &str) -> Result<(Dentition, Vec<u8>, bool, i64)> {
    let row: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT dentition, primary_slots, version FROM dental_chart WHERE patient_id = ?1",
            [patient_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    Ok(match row {
        Some((d, slots, v)) => (dentition_from(&d), parse_teeth(&slots), true, v),
        None => {
            let d = dentition_for_age(patient_age(conn, patient_id)?);
            (d, DEFAULT_PRIMARY_SLOTS.to_vec(), false, 0)
        }
    })
}

const FINDING_SELECT: &str =
    "SELECT f.id, f.patient_id, f.tooth, f.surfaces, f.condition_id, f.note, f.recorded_on,
    u.display_name, f.version
    FROM tooth_finding f LEFT JOIN app_user u ON u.id = f.created_by
    WHERE f.deleted_at IS NULL";

fn map_finding(r: &rusqlite::Row<'_>) -> rusqlite::Result<ToothFinding> {
    Ok(ToothFinding {
        id: r.get(0)?,
        patient_id: r.get(1)?,
        tooth: r.get(2)?,
        surfaces: r.get(3)?,
        condition_id: r.get(4)?,
        note: r.get(5)?,
        recorded_on: r.get(6)?,
        recorded_by_name: r.get(7)?,
        version: r.get(8)?,
    })
}

pub fn list_findings(conn: &Connection, patient_id: &str) -> Result<Vec<ToothFinding>> {
    let mut stmt = conn.prepare(&format!(
        "{FINDING_SELECT} AND f.patient_id = ?1 ORDER BY f.tooth, f.recorded_on, f.created_at"
    ))?;
    let v = stmt.query_map([patient_id], map_finding)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

fn get_finding(conn: &Connection, id: &str) -> Result<ToothFinding> {
    conn.query_row(&format!("{FINDING_SELECT} AND f.id = ?1"), [id], map_finding).optional()?.ok_or_else(
        || {
            CoreError::api(ErrorCode::NotFound, format!("finding {id}"))
                .on_field("id", ValidationRule::ConditionNotFound)
        },
    )
}

pub fn get(conn: &Connection, patient_id: &str) -> Result<ChartInfo> {
    let (dentition, primary_slots, chosen, version) = dentition_of(conn, patient_id)?;
    let mut stmt = conn.prepare(
        "SELECT id, title, status FROM treatment_plan WHERE patient_id = ?1 AND deleted_at IS NULL ORDER BY created_at",
    )?;
    let plans = stmt
        .query_map([patient_id], |r| {
            Ok(PlanBrief {
                id: r.get(0)?,
                title: r.get(1)?,
                status: crate::treatment::plan_status_from(&r.get::<_, String>(2)?),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ChartInfo {
        patient_id: patient_id.to_string(),
        dentition,
        primary_slots,
        dentition_chosen: chosen,
        findings: list_findings(conn, patient_id)?,
        items: crate::treatment::chart_items(conn, patient_id)?,
        plans,
        version,
    })
}

pub fn set_dentition(conn: &Connection, actor: &Actor, p: &SetDentitionParams) -> Result<ChartInfo> {
    crate::treatment::check_patient(conn, &p.patient_id)?;
    let slots = check_teeth("primary_slots", &p.primary_slots)?;
    if slots.iter().any(|&t| t > 48 || t % 10 > 5) {
        return Err(CoreError::invalid(
            "primary_slots",
            ValidationRule::DentitionSlots,
            "a baby tooth can only take the place of an adult tooth 1 to 5",
        ));
    }
    let (before, before_slots, chosen, version) = dentition_of(conn, &p.patient_id)?;
    let now = now_iso();
    if chosen {
        let changed = conn.execute(
            "UPDATE dental_chart SET dentition=?1, primary_slots=?2, updated_at=?3, updated_by=?4, version=version+1
             WHERE patient_id=?5 AND version=?6",
            params![dentition_code(p.dentition), teeth_text(&slots), now, actor.user_id, p.patient_id, p.version],
        )?;
        expect_one_row(changed, "dental chart")?;
    } else {
        conn.execute(
            "INSERT INTO dental_chart(patient_id, dentition, primary_slots, created_at, created_by, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?4, ?5)",
            params![p.patient_id, dentition_code(p.dentition), teeth_text(&slots), now, actor.user_id],
        )?;
    }
    audit::record(
        conn,
        actor,
        "chart.dentition",
        Some("patient"),
        Some(&p.patient_id),
        Some(
            &json!({"dentition": dentition_code(before), "primary_slots": before_slots, "version": version}),
        ),
        Some(&json!({"dentition": dentition_code(p.dentition), "primary_slots": slots})),
    )?;
    get(conn, &p.patient_id)
}

pub fn add_findings(conn: &Connection, actor: &Actor, p: &AddFindingsParams) -> Result<Vec<ToothFinding>> {
    crate::treatment::check_patient(conn, &p.patient_id)?;
    let condition = get_condition(conn, &p.condition_id)?;
    if !condition.is_active {
        return Err(CoreError::invalid(
            "condition_id",
            ValidationRule::ConditionNotFound,
            "this condition is switched off",
        ));
    }
    let teeth = check_teeth("teeth", &p.teeth)?;
    if teeth.is_empty() {
        return Err(CoreError::invalid("teeth", ValidationRule::ItemTeeth, "choose at least one tooth"));
    }
    let surfaces = clean_surfaces("surfaces", &p.surfaces)?;
    if condition.needs_surface && surfaces.is_empty() {
        return Err(CoreError::invalid("surfaces", ValidationRule::ToothSurface, "choose the surfaces"));
    }
    let note = p.note.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
    if note.as_deref().is_some_and(|n| n.chars().count() > 500) {
        return Err(CoreError::invalid(
            "note",
            ValidationRule::DocumentText,
            "a note is at most 500 characters",
        ));
    }
    let now = now_iso();
    let today = today_iso();
    let mut out = Vec::new();
    for t in teeth {
        let id = new_id();
        conn.execute(
            "INSERT INTO tooth_finding(id, patient_id, tooth, surfaces, condition_id, note, recorded_on, created_at,
                created_by, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?8, ?9)",
            params![id, p.patient_id, t, surfaces, condition.id, note, today, now, actor.user_id],
        )?;
        out.push(get_finding(conn, &id)?);
    }
    audit::record(
        conn,
        actor,
        "chart.finding_add",
        Some("patient"),
        Some(&p.patient_id),
        None,
        Some(
            &json!({"condition": condition.code, "teeth": out.iter().map(|f| f.tooth).collect::<Vec<_>>(), "surfaces": surfaces}),
        ),
    )?;
    Ok(out)
}

/// A finding entered by mistake (or no longer true) leaves the chart; the row stays, marked deleted.
pub fn remove_finding(conn: &Connection, actor: &Actor, id: &str, version: i64) -> Result<()> {
    let before = get_finding(conn, id)?;
    let now = now_iso();
    let changed = conn.execute(
        "UPDATE tooth_finding SET deleted_at=?1, deleted_by=?2, updated_at=?1, updated_by=?2, version=version+1
         WHERE id=?3 AND version=?4 AND deleted_at IS NULL",
        params![now, actor.user_id, id, version],
    )?;
    expect_one_row(changed, "finding")?;
    audit::record(
        conn,
        actor,
        "chart.finding_remove",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!(before)),
        None,
    )?;
    Ok(())
}

/// Everything that happened to one tooth: findings, planned work, sessions, completion.
pub fn tooth_history(conn: &Connection, patient_id: &str, tooth: u8) -> Result<Vec<TimelineEvent>> {
    if !is_tooth(tooth) {
        return Err(CoreError::invalid("tooth", ValidationRule::ToothNumber, "not an FDI tooth number"));
    }
    crate::timeline::events(conn, patient_id, &[], 500, Some(tooth))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fdi_numbers_and_surfaces() {
        assert!(is_tooth(11) && is_tooth(48) && is_tooth(55) && is_tooth(85));
        assert!(!is_tooth(19) && !is_tooth(56) && !is_tooth(10) && !is_tooth(90) && !is_tooth(9));
        assert_eq!(quadrant(65), 2);
        assert!(is_upper(26) && is_upper(61) && !is_upper(36) && !is_upper(84));
        assert_eq!(check_teeth("teeth", &[37, 36, 36]).unwrap(), vec![36, 37]);
        assert!(check_teeth("teeth", &[99]).is_err());
        assert_eq!(clean_surfaces("s", "dom").unwrap(), "MOD");
        assert_eq!(clean_surfaces("s", "I,F,P").unwrap(), "OBL");
        assert!(clean_surfaces("s", "X").is_err());
    }

    #[test]
    fn dentitions_list_their_teeth() {
        assert_eq!(all_teeth(Dentition::Permanent, &[]).len(), 32);
        let primary = all_teeth(Dentition::Primary, &[]);
        assert_eq!(primary.len(), 20);
        assert!(primary.iter().all(|&t| is_primary(t)));
        let mixed = all_teeth(Dentition::Mixed, &DEFAULT_PRIMARY_SLOTS);
        assert_eq!(mixed.len(), 32);
        assert!(mixed.contains(&53) && mixed.contains(&11) && mixed.contains(&16) && !mixed.contains(&13));
        assert_eq!(dentition_for_age(Some(4)), Dentition::Primary);
        assert_eq!(dentition_for_age(Some(9)), Dentition::Mixed);
        assert_eq!(dentition_for_age(None), Dentition::Permanent);
    }

    #[test]
    fn condition_seeds_are_valid() {
        let seeds = condition_seeds().unwrap();
        let mut codes = std::collections::HashSet::new();
        for c in &seeds {
            assert!(codes.insert(c.code.clone()), "duplicate {}", c.code);
            assert!(!c.name.fa.contains(['ي', 'ك']), "{}", c.code);
            assert!(c.symbol != ChartSymbol::Marker || c.abbreviation.is_some(), "{}", c.code);
        }
        for code in
            ["caries", "filling", "missing", "extracted", "crown", "root_canal", "implant", "bridge_pontic"]
        {
            assert!(codes.contains(code), "5.3 lists {code}");
        }
    }
}
