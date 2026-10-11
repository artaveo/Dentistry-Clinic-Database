//! Clinical notes of each visit (5.6): chief complaint, examination, diagnosis, findings, procedure,
//! notes, follow-up and recommendations. A note is a draft until the doctor signs it off; a signed
//! note is locked and later changes are addenda (dated, named, never edited). Ready-made notes come
//! from `core/seeds/note_templates.json` and follow the seed until the clinic rewrites them.

use artaveo_shared::{
    AddAddendumParams, ClinicalNoteInfo, ErrorCode, NoteAddendum, NoteFields, NoteStatus, NoteTemplateInfo,
    NoteTemplateTexts, SaveNoteParams, SaveNoteTemplateParams, Translations, ValidationRule,
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
use crate::scheduling::{format_date, parse_date};

const NOTE_TEMPLATES_JSON: &str = include_str!("../seeds/note_templates.json");
const FIELD_MAX: usize = 4000;

fn field_list(f: &NoteFields) -> [(&'static str, &String); 8] {
    [
        ("chief_complaint", &f.chief_complaint),
        ("examination", &f.examination),
        ("diagnosis", &f.diagnosis),
        ("findings", &f.findings),
        ("procedure", &f.procedure),
        ("notes", &f.notes),
        ("follow_up", &f.follow_up),
        ("recommendations", &f.recommendations),
    ]
}

/// Trims every part; each at most 4000 characters (the field error names the part).
fn clean_fields(prefix: &str, f: &NoteFields) -> Result<NoteFields> {
    for (name, v) in field_list(f) {
        if v.trim().chars().count() > FIELD_MAX {
            return Err(CoreError::invalid(
                &format!("{prefix}{name}"),
                ValidationRule::DocumentText,
                format!("{name} is at most {FIELD_MAX} characters"),
            ));
        }
    }
    let t = |s: &String| s.trim().to_string();
    Ok(NoteFields {
        chief_complaint: t(&f.chief_complaint),
        examination: t(&f.examination),
        diagnosis: t(&f.diagnosis),
        findings: t(&f.findings),
        procedure: t(&f.procedure),
        notes: t(&f.notes),
        follow_up: t(&f.follow_up),
        recommendations: t(&f.recommendations),
    })
}

fn is_empty(f: &NoteFields) -> bool {
    field_list(f).iter().all(|(_, v)| v.is_empty())
}

// ───────────────────────────── notes ─────────────────────────────

const NOTE_SELECT: &str = "SELECT n.id, n.patient_id, n.appointment_id, n.item_id, n.doctor_id, d.full_name, n.visit_date,
    n.chief_complaint, n.examination, n.diagnosis, n.findings, n.procedure, n.notes, n.follow_up, n.recommendations,
    n.template_id, n.status, n.signed_at, s.display_name, n.created_at, c.display_name, n.version
    FROM clinical_note n
    LEFT JOIN doctor d ON d.id = n.doctor_id
    LEFT JOIN app_user s ON s.id = n.signed_by
    LEFT JOIN app_user c ON c.id = n.created_by
    WHERE n.deleted_at IS NULL";

fn map_note(r: &rusqlite::Row<'_>) -> rusqlite::Result<ClinicalNoteInfo> {
    Ok(ClinicalNoteInfo {
        id: r.get(0)?,
        patient_id: r.get(1)?,
        appointment_id: r.get(2)?,
        item_id: r.get(3)?,
        doctor_id: r.get(4)?,
        doctor_name: r.get(5)?,
        visit_date: r.get(6)?,
        fields: NoteFields {
            chief_complaint: r.get(7)?,
            examination: r.get(8)?,
            diagnosis: r.get(9)?,
            findings: r.get(10)?,
            procedure: r.get(11)?,
            notes: r.get(12)?,
            follow_up: r.get(13)?,
            recommendations: r.get(14)?,
        },
        template_id: r.get(15)?,
        status: if r.get::<_, String>(16)? == "signed" { NoteStatus::Signed } else { NoteStatus::Draft },
        signed_at: r.get(17)?,
        signed_by_name: r.get(18)?,
        addenda: Vec::new(),
        created_at: r.get(19)?,
        created_by_name: r.get(20)?,
        version: r.get(21)?,
    })
}

fn with_addenda(conn: &Connection, mut n: ClinicalNoteInfo) -> Result<ClinicalNoteInfo> {
    let mut stmt = conn.prepare_cached(
        "SELECT a.id, a.text, a.created_at, u.display_name FROM clinical_note_addendum a
         LEFT JOIN app_user u ON u.id = a.created_by WHERE a.note_id = ?1 ORDER BY a.created_at",
    )?;
    n.addenda = stmt
        .query_map([&n.id], |r| {
            Ok(NoteAddendum {
                id: r.get(0)?,
                text: r.get(1)?,
                created_at: r.get(2)?,
                created_by_name: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(n)
}

pub fn get(conn: &Connection, id: &str) -> Result<ClinicalNoteInfo> {
    let n = conn.query_row(&format!("{NOTE_SELECT} AND n.id = ?1"), [id], map_note).optional()?.ok_or_else(
        || {
            CoreError::api(ErrorCode::NotFound, format!("clinical note {id}"))
                .on_field("id", ValidationRule::NoteNotFound)
        },
    )?;
    with_addenda(conn, n)
}

/// A patient's notes, newest visit first.
pub fn list(conn: &Connection, patient_id: &str) -> Result<Vec<ClinicalNoteInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{NOTE_SELECT} AND n.patient_id = ?1 ORDER BY n.visit_date DESC, n.created_at DESC"
    ))?;
    let rows: Vec<ClinicalNoteInfo> =
        stmt.query_map([patient_id], map_note)?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|n| with_addenda(conn, n)).collect()
}

pub fn save(conn: &Connection, actor: &Actor, p: &SaveNoteParams) -> Result<ClinicalNoteInfo> {
    crate::treatment::check_patient(conn, &p.patient_id)?;
    let fields = clean_fields("fields.", &p.fields)?;
    if is_empty(&fields) {
        return Err(CoreError::invalid(
            "fields",
            ValidationRule::NoteEmpty,
            "write at least one part of the note",
        ));
    }
    if let Some(a) = p.appointment_id.as_deref().filter(|a| !a.is_empty()) {
        let appt =
            crate::appointment::get_appointment(conn, a).map_err(|e| e.rename_field("appointment_id"))?;
        if appt.patient_id != p.patient_id {
            return Err(CoreError::invalid(
                "appointment_id",
                ValidationRule::ItemPatient,
                "that visit is another patient's",
            ));
        }
    }
    if let Some(i) = p.item_id.as_deref().filter(|a| !a.is_empty()) {
        let item = crate::treatment::get_item(conn, i).map_err(|e| e.rename_field("item_id"))?;
        if item.patient_id != p.patient_id {
            return Err(CoreError::invalid(
                "item_id",
                ValidationRule::ItemPatient,
                "that treatment is another patient's",
            ));
        }
    }
    let doctor = match p.doctor_id.as_deref().filter(|d| !d.is_empty()) {
        Some(d) => Some(crate::scheduling::get_doctor(conn, d).map_err(|e| e.rename_field("doctor_id"))?.id),
        None => None,
    };
    let today = today_iso();
    let visit_date = match p.visit_date.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => format_date(parse_date("visit_date", d)?),
        None => today.clone(),
    };
    if visit_date > today {
        return Err(CoreError::invalid(
            "visit_date",
            ValidationRule::DateFormat,
            "a visit note cannot be in the future",
        ));
    }
    let template = p.template_id.as_deref().filter(|t| !t.is_empty());
    let now = now_iso();
    let status = if p.sign { "signed" } else { "draft" };
    let signed_at = p.sign.then(|| now.clone());
    let signed_by = if p.sign { actor.user_id.clone() } else { None };
    let appointment = p.appointment_id.as_deref().filter(|a| !a.is_empty());
    let item = p.item_id.as_deref().filter(|a| !a.is_empty());
    let (id, before) = match &p.id {
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO clinical_note(id, patient_id, appointment_id, item_id, doctor_id, visit_date, chief_complaint,
                    examination, diagnosis, findings, procedure, notes, follow_up, recommendations, template_id, status,
                    signed_at, signed_by, created_at, created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?19, ?20)",
                params![
                    id,
                    p.patient_id,
                    appointment,
                    item,
                    doctor,
                    visit_date,
                    fields.chief_complaint,
                    fields.examination,
                    fields.diagnosis,
                    fields.findings,
                    fields.procedure,
                    fields.notes,
                    fields.follow_up,
                    fields.recommendations,
                    template,
                    status,
                    signed_at,
                    signed_by,
                    now,
                    actor.user_id
                ],
            )?;
            (id, None)
        }
        Some(id) => {
            let before = get(conn, id).map_err(|e| e.rename_field("id"))?;
            if before.status == NoteStatus::Signed {
                return Err(CoreError::invalid(
                    "id",
                    ValidationRule::NoteSigned,
                    "a signed note is changed only by addenda",
                ));
            }
            if before.patient_id != p.patient_id {
                return Err(CoreError::invalid(
                    "patient_id",
                    ValidationRule::ItemPatient,
                    "the note is another patient's",
                ));
            }
            let changed = conn.execute(
                "UPDATE clinical_note SET appointment_id=?1, item_id=?2, doctor_id=?3, visit_date=?4, chief_complaint=?5,
                    examination=?6, diagnosis=?7, findings=?8, procedure=?9, notes=?10, follow_up=?11, recommendations=?12,
                    template_id=?13, status=?14, signed_at=?15, signed_by=?16, updated_at=?17, updated_by=?18,
                    version=version+1
                 WHERE id=?19 AND version=?20 AND deleted_at IS NULL AND status='draft'",
                params![
                    appointment,
                    item,
                    doctor,
                    visit_date,
                    fields.chief_complaint,
                    fields.examination,
                    fields.diagnosis,
                    fields.findings,
                    fields.procedure,
                    fields.notes,
                    fields.follow_up,
                    fields.recommendations,
                    template,
                    status,
                    signed_at,
                    signed_by,
                    now,
                    actor.user_id,
                    id,
                    p.version
                ],
            )?;
            expect_one_row(changed, "clinical note")?;
            (id.clone(), Some(before))
        }
    };
    let after = get(conn, &id)?;
    let action = match (&before, p.sign) {
        (_, true) => "note.sign",
        (None, false) => "note.create",
        (Some(_), false) => "note.update",
    };
    audit::record(
        conn,
        actor,
        action,
        Some("patient"),
        Some(&p.patient_id),
        before.as_ref().map(|b| json!({"note_id": b.id, "fields": b.fields})).as_ref(),
        Some(&json!({"note_id": after.id, "status": after.status, "fields": after.fields})),
    )?;
    Ok(after)
}

/// Signs off a draft: from now on it is locked.
pub fn sign(conn: &Connection, actor: &Actor, id: &str, version: i64) -> Result<ClinicalNoteInfo> {
    let before = get(conn, id).map_err(|e| e.rename_field("id"))?;
    if before.status == NoteStatus::Signed {
        return Err(CoreError::invalid("id", ValidationRule::NoteSigned, "this note is already signed"));
    }
    if is_empty(&before.fields) {
        return Err(CoreError::invalid(
            "fields",
            ValidationRule::NoteEmpty,
            "an empty note cannot be signed",
        ));
    }
    let now = now_iso();
    let changed = conn.execute(
        "UPDATE clinical_note SET status='signed', signed_at=?1, signed_by=?2, updated_at=?1, updated_by=?2,
            version=version+1
         WHERE id=?3 AND version=?4 AND deleted_at IS NULL AND status='draft'",
        params![now, actor.user_id, id, version],
    )?;
    expect_one_row(changed, "clinical note")?;
    let after = get(conn, id)?;
    audit::record(
        conn,
        actor,
        "note.sign",
        Some("patient"),
        Some(&before.patient_id),
        None,
        Some(&json!({"note_id": id, "fields": after.fields})),
    )?;
    Ok(after)
}

pub fn add_addendum(conn: &Connection, actor: &Actor, p: &AddAddendumParams) -> Result<ClinicalNoteInfo> {
    let note = get(conn, &p.note_id).map_err(|e| e.rename_field("note_id"))?;
    let text = p.text.trim();
    if text.is_empty() {
        return Err(CoreError::invalid("text", ValidationRule::Required, "write the addendum"));
    }
    if text.chars().count() > 2000 {
        return Err(CoreError::invalid(
            "text",
            ValidationRule::DocumentText,
            "an addendum is at most 2000 characters",
        ));
    }
    conn.execute(
        "INSERT INTO clinical_note_addendum(id, note_id, text, created_at, created_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![new_id(), note.id, text, now_iso(), actor.user_id],
    )?;
    audit::record(
        conn,
        actor,
        "note.addendum",
        Some("patient"),
        Some(&note.patient_id),
        None,
        Some(&json!({"note_id": note.id, "text": text})),
    )?;
    get(conn, &note.id)
}

/// Throws away a draft (a signed note is part of the record and stays).
pub fn delete(conn: &Connection, actor: &Actor, id: &str, version: i64) -> Result<()> {
    let before = get(conn, id).map_err(|e| e.rename_field("id"))?;
    if before.status == NoteStatus::Signed {
        return Err(CoreError::invalid("id", ValidationRule::NoteSigned, "a signed note cannot be deleted"));
    }
    let now = now_iso();
    let changed = conn.execute(
        "UPDATE clinical_note SET deleted_at=?1, updated_at=?1, updated_by=?2, version=version+1
         WHERE id=?3 AND version=?4 AND deleted_at IS NULL AND status='draft'",
        params![now, actor.user_id, id, version],
    )?;
    expect_one_row(changed, "clinical note")?;
    audit::record(
        conn,
        actor,
        "note.delete",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!({"note_id": id, "fields": before.fields})),
        None,
    )?;
    Ok(())
}

// ───────────────────────────── ready-made notes ─────────────────────────────

#[derive(Deserialize)]
struct TemplateSeed {
    code: String,
    name: Translations,
    body: NoteTemplateTexts,
}

fn template_seeds() -> Result<Vec<TemplateSeed>> {
    serde_json::from_str(NOTE_TEMPLATES_JSON)
        .map_err(|e| CoreError::validation(format!("note_templates.json: {e}")))
}

/// Built-in note templates follow the seed until the clinic rewrites them (`customized`).
pub fn sync_seeds(conn: &Connection) -> Result<()> {
    let now = now_iso();
    for (n, t) in template_seeds()?.iter().enumerate() {
        let id = seed_id("note_template", &t.code);
        let customized: Option<bool> = conn
            .query_row("SELECT customized FROM note_template WHERE id = ?1", [&id], |r| r.get(0))
            .optional()?;
        let body = serde_json::to_string(&t.body)?;
        match customized {
            Some(true) => {}
            Some(false) => {
                conn.execute(
                    "UPDATE note_template SET name_fa=?1, name_ps=?2, name_en=?3, body_json=?4 WHERE id=?5",
                    params![t.name.fa, t.name.ps, t.name.en, body, id],
                )?;
            }
            None => {
                conn.execute(
                    "INSERT INTO note_template(id, code, name_fa, name_ps, name_en, body_json, sort_order, is_system,
                        created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8)",
                    params![id, t.code, t.name.fa, t.name.ps, t.name.en, body, (n as i64 + 1) * 10, now],
                )?;
            }
        }
    }
    Ok(())
}

const TEMPLATE_SELECT: &str =
    "SELECT id, code, name_fa, name_ps, name_en, body_json, is_system, customized, is_active,
    version FROM note_template WHERE deleted_at IS NULL";

fn map_template(r: &rusqlite::Row<'_>) -> rusqlite::Result<NoteTemplateInfo> {
    let body: String = r.get(5)?;
    Ok(NoteTemplateInfo {
        id: r.get(0)?,
        code: r.get(1)?,
        name: Translations { fa: r.get(2)?, ps: r.get(3)?, en: r.get(4)? },
        body: serde_json::from_str(&body).unwrap_or_default(),
        is_system: r.get(6)?,
        customized: r.get(7)?,
        is_active: r.get(8)?,
        version: r.get(9)?,
    })
}

pub fn list_templates(conn: &Connection, include_inactive: bool) -> Result<Vec<NoteTemplateInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{TEMPLATE_SELECT} AND (?1 = 1 OR is_active = 1) ORDER BY sort_order, created_at"
    ))?;
    let v = stmt.query_map([include_inactive], map_template)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn get_template(conn: &Connection, id: &str) -> Result<NoteTemplateInfo> {
    conn.query_row(&format!("{TEMPLATE_SELECT} AND id = ?1"), [id], map_template).optional()?.ok_or_else(
        || {
            CoreError::api(ErrorCode::NotFound, format!("note template {id}"))
                .on_field("id", ValidationRule::NoteTemplateNotFound)
        },
    )
}

pub fn save_template(
    conn: &Connection,
    actor: &Actor,
    p: &SaveNoteTemplateParams,
) -> Result<NoteTemplateInfo> {
    let name = clean_translations("name", &p.name, 120)?;
    let body = NoteTemplateTexts {
        fa: clean_fields("body.fa.", &p.body.fa)?,
        ps: clean_fields("body.ps.", &p.body.ps)?,
        en: clean_fields("body.en.", &p.body.en)?,
    };
    if is_empty(&body.fa) && is_empty(&body.ps) && is_empty(&body.en) {
        return Err(CoreError::invalid(
            "body",
            ValidationRule::NoteEmpty,
            "a ready-made note needs some text",
        ));
    }
    // An empty language takes the first one written, like other clinic-editable texts.
    let first =
        [&body.fa, &body.ps, &body.en].into_iter().find(|f| !is_empty(f)).cloned().unwrap_or_default();
    let fill = |f: NoteFields| if is_empty(&f) { first.clone() } else { f };
    let body =
        NoteTemplateTexts { fa: fill(body.fa.clone()), ps: fill(body.ps.clone()), en: fill(body.en.clone()) };
    let body_json = serde_json::to_string(&body)?;
    let now = now_iso();
    let (id, before) = match &p.id {
        None => {
            let id = new_id();
            let sort: i64 =
                conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 10 FROM note_template", [], |r| {
                    r.get(0)
                })?;
            conn.execute(
                "INSERT INTO note_template(id, code, name_fa, name_ps, name_en, body_json, sort_order, is_active, created_at,
                    created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?9, ?10)",
                params![
                    id,
                    format!("custom_{}", id.replace('-', "")),
                    name.fa,
                    name.ps,
                    name.en,
                    body_json,
                    sort,
                    p.is_active,
                    now,
                    actor.user_id
                ],
            )?;
            (id, None)
        }
        Some(id) => {
            let before = get_template(conn, id)?;
            let customized =
                before.customized || (before.is_system && (before.name != name || before.body != body));
            let changed = conn.execute(
                "UPDATE note_template SET name_fa=?1, name_ps=?2, name_en=?3, body_json=?4, is_active=?5, customized=?6,
                    updated_at=?7, updated_by=?8, version=version+1
                 WHERE id=?9 AND version=?10 AND deleted_at IS NULL",
                params![name.fa, name.ps, name.en, body_json, p.is_active, customized, now, actor.user_id, id, p.version],
            )?;
            expect_one_row(changed, "note template")?;
            (id.clone(), Some(before))
        }
    };
    let after = get_template(conn, &id)?;
    audit::record(
        conn,
        actor,
        if before.is_some() { "note_template.update" } else { "note_template.create" },
        Some("note_template"),
        Some(&id),
        before.as_ref().map(|b| json!(b)).as_ref(),
        Some(&json!(after)),
    )?;
    Ok(after)
}

/// Puts a rewritten built-in note back to the words that ship with the app.
pub fn reset_template(conn: &Connection, actor: &Actor, id: &str, version: i64) -> Result<NoteTemplateInfo> {
    let before = get_template(conn, id)?;
    let seed =
        template_seeds()?.into_iter().find(|t| seed_id("note_template", &t.code) == id).ok_or_else(|| {
            CoreError::invalid(
                "id",
                ValidationRule::NoteTemplateNotFound,
                "only built-in notes have a default",
            )
        })?;
    let changed = conn.execute(
        "UPDATE note_template SET name_fa=?1, name_ps=?2, name_en=?3, body_json=?4, customized=0, updated_at=?5,
            updated_by=?6, version=version+1
         WHERE id=?7 AND version=?8 AND deleted_at IS NULL",
        params![seed.name.fa, seed.name.ps, seed.name.en, serde_json::to_string(&seed.body)?, now_iso(), actor.user_id, id, version],
    )?;
    expect_one_row(changed, "note template")?;
    let after = get_template(conn, id)?;
    audit::record(
        conn,
        actor,
        "note_template.reset",
        Some("note_template"),
        Some(id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_template_seeds_are_valid() {
        let seeds = template_seeds().unwrap();
        assert!(seeds.len() >= 8);
        let mut codes = std::collections::HashSet::new();
        for t in &seeds {
            assert!(codes.insert(t.code.clone()), "duplicate {}", t.code);
            for f in [&t.body.fa, &t.body.ps, &t.body.en] {
                assert!(!is_empty(f), "{}", t.code);
            }
            for (_, v) in field_list(&t.body.fa) {
                assert!(!v.contains(['ي', 'ك']), "{}: Dari uses Persian ی/ک", t.code);
            }
        }
    }
}
