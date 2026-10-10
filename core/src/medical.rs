//! Structured medical history (M1 of docs/specs/clinical-workflow-v1.md, OF-025): a checklist of
//! yes / no / unknown questions instead of free text. Questions are reference data — the system ones
//! come from `core/seeds/medical_questions.psv` (see `seeds::sync`), the clinic can switch them off
//! and add its own — and a patient's answers are replaced as a whole on every save, guarded by the
//! version of the patient's history row, which also holds the «سایر توضیحات» notes and the date the
//! history was last reviewed.

use std::collections::HashMap;

use artaveo_shared::{
    CreateMedicalQuestionParams, ErrorCode, MedicalAnswer, MedicalAnswerValue, MedicalDetailKind,
    MedicalHistoryInfo, MedicalQuestionInfo, ReviewMedicalHistoryParams, Translations,
    UpdateMedicalHistoryParams, UpdateMedicalQuestionParams, ValidationRule,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::clock::{now, now_iso, parse};
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::new_id;

/// Checklist groups, in the order they are shown.
pub const GROUPS: [&str; 17] = [
    "cardio",
    "blood",
    "endocrine",
    "respiratory",
    "infectious",
    "kidney_liver",
    "neuro",
    "bone",
    "cancer",
    "digestive",
    "skin",
    "habits",
    "women",
    "allergy",
    "medication",
    "surgery",
    "other",
];

/// A history not reviewed for longer than this is asked again at the next visit (M1).
pub const REVIEW_AFTER_DAYS: i64 = 182;

const MAX_LABEL: usize = 200;
const MAX_DETAIL: usize = 300;
const MAX_NOTES: usize = 4000;

pub(crate) fn detail_kind_code(k: MedicalDetailKind) -> &'static str {
    match k {
        MedicalDetailKind::None => "none",
        MedicalDetailKind::Text => "text",
        MedicalDetailKind::Choice => "choice",
        MedicalDetailKind::TextChoice => "text_choice",
        MedicalDetailKind::Months => "months",
    }
}

fn detail_kind_from(s: &str) -> MedicalDetailKind {
    match s {
        "text" => MedicalDetailKind::Text,
        "choice" => MedicalDetailKind::Choice,
        "text_choice" => MedicalDetailKind::TextChoice,
        "months" => MedicalDetailKind::Months,
        _ => MedicalDetailKind::None,
    }
}

fn answer_code(a: MedicalAnswerValue) -> &'static str {
    match a {
        MedicalAnswerValue::Yes => "yes",
        MedicalAnswerValue::No => "no",
        MedicalAnswerValue::Unknown => "unknown",
    }
}

fn answer_from(s: &str) -> MedicalAnswerValue {
    match s {
        "yes" => MedicalAnswerValue::Yes,
        "no" => MedicalAnswerValue::No,
        _ => MedicalAnswerValue::Unknown,
    }
}

// ───────────────────────────── questions ─────────────────────────────

type Labels = HashMap<String, (Translations, Option<Translations>, Option<Translations>)>;

/// All three languages of every question's label, detail label and alert note.
fn labels(conn: &Connection) -> Result<Labels> {
    let mut stmt = conn
        .prepare("SELECT question_id, language_code, label, detail_label, alert_note FROM medical_question_translation")?;
    let mut out: Labels = HashMap::new();
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    for row in rows {
        let (id, lang, label, detail, note) = row?;
        let e = out.entry(id).or_default();
        let set = |t: &mut Translations, v: String| match lang.as_str() {
            "fa" => t.fa = v,
            "ps" => t.ps = v,
            _ => t.en = v,
        };
        set(&mut e.0, label);
        if let Some(d) = detail {
            set(e.1.get_or_insert_with(Translations::default), d);
        }
        if let Some(n) = note {
            set(e.2.get_or_insert_with(Translations::default), n);
        }
    }
    Ok(out)
}

const QUESTION_SELECT: &str =
    "SELECT id, code, group_code, sort_order, detail_kind, choices, alert, female_only,
    is_system, is_active, version FROM medical_question WHERE deleted_at IS NULL";

fn map_question(r: &rusqlite::Row<'_>) -> rusqlite::Result<MedicalQuestionInfo> {
    let choices: Option<String> = r.get(5)?;
    Ok(MedicalQuestionInfo {
        id: r.get(0)?,
        code: r.get(1)?,
        group_code: r.get(2)?,
        sort_order: r.get(3)?,
        detail_kind: detail_kind_from(&r.get::<_, String>(4)?),
        choices: choices.map(|c| c.split(';').map(str::to_string).collect()).unwrap_or_default(),
        alert: r.get(6)?,
        female_only: r.get(7)?,
        is_system: r.get(8)?,
        is_active: r.get(9)?,
        label: Translations::default(),
        detail_label: None,
        alert_note: None,
        version: r.get(10)?,
    })
}

fn with_labels(mut q: MedicalQuestionInfo, all: &Labels) -> MedicalQuestionInfo {
    if let Some((label, detail, note)) = all.get(&q.id) {
        q.label = label.clone();
        q.detail_label = detail.clone();
        q.alert_note = note.clone();
    }
    q
}

/// Checklist order: by group (as in [`GROUPS`]), then by the question's own order.
fn group_rank(code: &str) -> usize {
    GROUPS.iter().position(|g| *g == code).unwrap_or(GROUPS.len())
}

pub fn list_questions(conn: &Connection, include_inactive: bool) -> Result<Vec<MedicalQuestionInfo>> {
    let all = labels(conn)?;
    let mut stmt = conn.prepare(&format!("{QUESTION_SELECT} AND (?1 = 1 OR is_active = 1)"))?;
    let mut v: Vec<MedicalQuestionInfo> = stmt
        .query_map([include_inactive as i64], map_question)?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .map(|q| with_labels(q, &all))
        .collect();
    v.sort_by(|a, b| {
        (group_rank(&a.group_code), a.sort_order, &a.code).cmp(&(
            group_rank(&b.group_code),
            b.sort_order,
            &b.code,
        ))
    });
    Ok(v)
}

pub fn get_question(conn: &Connection, id: &str) -> Result<MedicalQuestionInfo> {
    let q = conn
        .query_row(&format!("{QUESTION_SELECT} AND id = ?1"), [id], map_question)
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("medical question {id}"))
                .on_field("id", ValidationRule::MedicalQuestionNotFound)
        })?;
    Ok(with_labels(q, &labels(conn)?))
}

/// The three labels of a clinic question: at least one given, each at most 200 characters; an empty
/// language takes the first one given so the checklist never shows a blank line.
fn clean_label(t: &Translations) -> Result<Translations> {
    let parts = [t.fa.trim(), t.ps.trim(), t.en.trim()];
    let Some(first) = parts.iter().find(|p| !p.is_empty()) else {
        return Err(CoreError::invalid(
            "label",
            ValidationRule::MedicalQuestionLabel,
            "a question needs a label",
        ));
    };
    if parts.iter().any(|p| p.chars().count() > MAX_LABEL) {
        return Err(CoreError::invalid(
            "label",
            ValidationRule::MedicalQuestionLabel,
            "a question label is at most 200 characters",
        ));
    }
    let or = |p: &str| if p.is_empty() { first.to_string() } else { p.to_string() };
    Ok(Translations { fa: or(parts[0]), ps: or(parts[1]), en: or(parts[2]) })
}

fn check_group(group: &str) -> Result<()> {
    if GROUPS.contains(&group) {
        Ok(())
    } else {
        Err(CoreError::invalid(
            "group_code",
            ValidationRule::MedicalQuestionGroup,
            format!("unknown group {group}"),
        ))
    }
}

/// A clinic question's detail can only be nothing or a short text (choices and months are system-only).
fn check_custom_detail(k: MedicalDetailKind) -> Result<()> {
    match k {
        MedicalDetailKind::None | MedicalDetailKind::Text => Ok(()),
        _ => Err(CoreError::invalid(
            "detail_kind",
            ValidationRule::MedicalDetailKind,
            "a clinic question can ask for a text detail or none",
        )),
    }
}

fn write_labels(conn: &Connection, id: &str, label: &Translations, detail: bool) -> Result<()> {
    // A clinic question's detail is a short free text: its label is the generic "details".
    let detail_label =
        Translations { fa: "توضیح".into(), ps: "تشریح".into(), en: "Details".into() };
    for (lang, text, d) in [
        ("fa", &label.fa, &detail_label.fa),
        ("ps", &label.ps, &detail_label.ps),
        ("en", &label.en, &detail_label.en),
    ] {
        conn.execute(
            "INSERT INTO medical_question_translation(question_id, language_code, label, detail_label, alert_note)
             VALUES (?1, ?2, ?3, ?4, NULL)
             ON CONFLICT(question_id, language_code) DO UPDATE SET label = excluded.label, detail_label = excluded.detail_label",
            params![id, lang, text, detail.then_some(d)],
        )?;
    }
    Ok(())
}

pub fn create_question(
    conn: &Connection,
    actor: &Actor,
    p: &CreateMedicalQuestionParams,
) -> Result<MedicalQuestionInfo> {
    check_group(&p.group_code)?;
    check_custom_detail(p.detail_kind)?;
    let label = clean_label(&p.label)?;
    let id = new_id();
    let now = now_iso();
    // After the group's last question, in steps of 10 like the seed.
    let sort: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 10 FROM medical_question WHERE group_code = ?1",
        [&p.group_code],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO medical_question(id, code, group_code, sort_order, detail_kind, alert, female_only, is_system,
            created_at, created_by, updated_at, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?8, ?9)",
        params![
            id,
            format!("custom_{}", id.replace('-', "")),
            p.group_code,
            sort,
            detail_kind_code(p.detail_kind),
            p.alert,
            p.female_only,
            now,
            actor.user_id
        ],
    )?;
    write_labels(conn, &id, &label, p.detail_kind == MedicalDetailKind::Text)?;
    let q = get_question(conn, &id)?;
    audit::record(
        conn,
        actor,
        "medical_question.create",
        Some("medical_question"),
        Some(&id),
        None,
        Some(&json!(q)),
    )?;
    Ok(q)
}

pub fn update_question(
    conn: &Connection,
    actor: &Actor,
    p: &UpdateMedicalQuestionParams,
) -> Result<MedicalQuestionInfo> {
    let before = get_question(conn, &p.id)?;
    let now = now_iso();
    let changed = if before.is_system {
        // A system question keeps its wording and meaning (prescriptions rely on its code and alert);
        // the clinic can only switch it off or move it.
        let reworded = p.group_code != before.group_code
            || p.detail_kind != before.detail_kind
            || p.alert != before.alert
            || p.female_only != before.female_only
            || clean_label(&p.label).ok().as_ref() != Some(&before.label);
        if reworded {
            return Err(CoreError::invalid(
                "id",
                ValidationRule::MedicalSystemQuestion,
                "a system question can only be switched off or reordered",
            ));
        }
        conn.execute(
            "UPDATE medical_question SET is_active=?1, sort_order=?2, updated_at=?3, updated_by=?4, version=version+1
             WHERE id=?5 AND version=?6 AND deleted_at IS NULL",
            params![p.is_active, p.sort_order, now, actor.user_id, p.id, p.version],
        )?
    } else {
        check_group(&p.group_code)?;
        check_custom_detail(p.detail_kind)?;
        let label = clean_label(&p.label)?;
        let n = conn.execute(
            "UPDATE medical_question SET group_code=?1, detail_kind=?2, alert=?3, female_only=?4, is_active=?5,
                sort_order=?6, updated_at=?7, updated_by=?8, version=version+1
             WHERE id=?9 AND version=?10 AND deleted_at IS NULL",
            params![
                p.group_code,
                detail_kind_code(p.detail_kind),
                p.alert,
                p.female_only,
                p.is_active,
                p.sort_order,
                now,
                actor.user_id,
                p.id,
                p.version
            ],
        )?;
        if n == 1 {
            write_labels(conn, &p.id, &label, p.detail_kind == MedicalDetailKind::Text)?;
        }
        n
    };
    expect_one_row(changed, "medical question")?;
    let after = get_question(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        "medical_question.update",
        Some("medical_question"),
        Some(&p.id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

// ───────────────────────────── a patient's history ─────────────────────────────

fn review_due(reviewed_at: Option<&str>) -> bool {
    match reviewed_at.and_then(parse) {
        Some(at) => now() - at > time::Duration::days(REVIEW_AFTER_DAYS),
        None => true,
    }
}

/// The answers of one patient, in checklist order.
fn answers(conn: &Connection, patient_id: &str) -> Result<Vec<MedicalAnswer>> {
    let mut stmt = conn.prepare(
        "SELECT a.question_id, a.answer, a.detail_text, a.detail_choice, q.group_code, q.sort_order, q.code
         FROM patient_medical_answer a JOIN medical_question q ON q.id = a.question_id
         WHERE a.patient_id = ?1",
    )?;
    let mut rows: Vec<(usize, i64, String, MedicalAnswer)> = stmt
        .query_map([patient_id], |r| {
            Ok((
                group_rank(&r.get::<_, String>(4)?),
                r.get(5)?,
                r.get(6)?,
                MedicalAnswer {
                    question_id: r.get(0)?,
                    answer: answer_from(&r.get::<_, String>(1)?),
                    detail_text: r.get(2)?,
                    detail_choice: r.get(3)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    rows.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
    Ok(rows.into_iter().map(|r| r.3).collect())
}

pub fn get_history(conn: &Connection, patient_id: &str) -> Result<MedicalHistoryInfo> {
    let row = conn
        .query_row(
            "SELECT h.notes, h.reviewed_at, u.display_name, h.version
             FROM patient_medical_history h LEFT JOIN app_user u ON u.id = h.reviewed_by
             WHERE h.patient_id = ?1 AND h.deleted_at IS NULL",
            [patient_id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;
    let (notes, reviewed_at, reviewed_by_name, version) = row.unwrap_or((None, None, None, 0));
    Ok(MedicalHistoryInfo {
        patient_id: patient_id.to_string(),
        answers: answers(conn, patient_id)?,
        notes,
        review_due: review_due(reviewed_at.as_deref()),
        reviewed_at,
        reviewed_by_name,
        version,
    })
}

/// Codes of the questions this patient answered "yes" to (prescription warnings, 5.9), with the
/// detail text (e.g. the months of a pregnancy).
pub fn yes_answers(conn: &Connection, patient_id: &str) -> Result<HashMap<String, Option<String>>> {
    let mut stmt = conn.prepare(
        "SELECT q.code, a.detail_text FROM patient_medical_answer a JOIN medical_question q ON q.id = a.question_id
         WHERE a.patient_id = ?1 AND a.answer = 'yes'",
    )?;
    let v = stmt.query_map([patient_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

fn trimmed(s: &Option<String>) -> Option<String> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// Checks one answer against its question and drops what a non-"yes" answer cannot carry.
fn clean_answer(q: &MedicalQuestionInfo, a: &MedicalAnswer) -> Result<MedicalAnswer> {
    let field = format!("answers.{}", q.id);
    if a.answer != MedicalAnswerValue::Yes {
        return Ok(MedicalAnswer {
            question_id: q.id.clone(),
            answer: a.answer,
            detail_text: None,
            detail_choice: None,
        });
    }
    let text = trimmed(&a.detail_text);
    let choice = trimmed(&a.detail_choice);
    let (text, choice) = match q.detail_kind {
        MedicalDetailKind::None => (None, None),
        MedicalDetailKind::Text => (text, None),
        MedicalDetailKind::Choice => (None, choice),
        MedicalDetailKind::TextChoice => (text, choice),
        MedicalDetailKind::Months => {
            let months = match text.as_deref() {
                None => None,
                Some(t) => {
                    let n: i64 = crate::calendar::digits::to_latin(t).trim().parse().map_err(|_| {
                        CoreError::invalid(
                            &field,
                            ValidationRule::MedicalMonthsRange,
                            "months must be a number",
                        )
                    })?;
                    if !(1..=10).contains(&n) {
                        return Err(CoreError::invalid(
                            &field,
                            ValidationRule::MedicalMonthsRange,
                            "months must be 1 to 10",
                        ));
                    }
                    Some(n.to_string())
                }
            };
            (months, None)
        }
    };
    if text.as_deref().is_some_and(|t| t.chars().count() > MAX_DETAIL) {
        return Err(CoreError::invalid(
            &field,
            ValidationRule::MedicalDetailLength,
            "a detail is at most 300 characters",
        ));
    }
    if let Some(c) = &choice {
        if !q.choices.contains(c) {
            return Err(CoreError::invalid(
                &field,
                ValidationRule::MedicalChoice,
                format!("unknown choice {c}"),
            ));
        }
    }
    Ok(MedicalAnswer {
        question_id: q.id.clone(),
        answer: a.answer,
        detail_text: text,
        detail_choice: choice,
    })
}

/// Creates the history row (version 0 → 1) or moves its version on, refusing a stale view (Conflict).
fn bump(
    conn: &Connection,
    actor: &Actor,
    patient_id: &str,
    version: i64,
    notes: Option<&Option<String>>,
) -> Result<()> {
    let now = now_iso();
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM patient_medical_history WHERE patient_id = ?1 AND deleted_at IS NULL)",
        [patient_id],
        |r| r.get(0),
    )?;
    if !exists {
        if version != 0 {
            return Err(CoreError::api(ErrorCode::Conflict, "medical history was changed by another user"));
        }
        conn.execute(
            "INSERT INTO patient_medical_history(id, patient_id, notes, reviewed_at, reviewed_by, created_at, created_by,
                updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?4, ?5, ?4, ?5)",
            params![new_id(), patient_id, notes.cloned().flatten(), now, actor.user_id],
        )?;
        return Ok(());
    }
    let changed = match notes {
        Some(n) => conn.execute(
            "UPDATE patient_medical_history SET notes=?1, reviewed_at=?2, reviewed_by=?3, updated_at=?2, updated_by=?3,
                version=version+1
             WHERE patient_id=?4 AND version=?5 AND deleted_at IS NULL",
            params![n, now, actor.user_id, patient_id, version],
        )?,
        None => conn.execute(
            "UPDATE patient_medical_history SET reviewed_at=?1, reviewed_by=?2, updated_at=?1, updated_by=?2,
                version=version+1
             WHERE patient_id=?3 AND version=?4 AND deleted_at IS NULL",
            params![now, actor.user_id, patient_id, version],
        )?,
    };
    expect_one_row(changed, "medical history")
}

/// Saves the whole checklist (a question left out = not asked) and the notes; saving also counts as
/// a review of the history.
pub fn update_history(
    conn: &Connection,
    actor: &Actor,
    p: &UpdateMedicalHistoryParams,
) -> Result<MedicalHistoryInfo> {
    crate::patient::get_patient(conn, &p.patient_id)?;
    let notes = trimmed(&p.notes);
    if notes.as_deref().is_some_and(|n| n.chars().count() > MAX_NOTES) {
        return Err(CoreError::invalid(
            "notes",
            ValidationRule::MedicalDetailLength,
            "notes are at most 4000 characters",
        ));
    }
    let questions: HashMap<String, MedicalQuestionInfo> =
        list_questions(conn, true)?.into_iter().map(|q| (q.id.clone(), q)).collect();
    let mut seen = std::collections::HashSet::new();
    let mut clean = Vec::with_capacity(p.answers.len());
    for a in &p.answers {
        let q = questions.get(&a.question_id).ok_or_else(|| {
            CoreError::invalid(
                &format!("answers.{}", a.question_id),
                ValidationRule::MedicalQuestionNotFound,
                "unknown question",
            )
        })?;
        if !seen.insert(a.question_id.clone()) {
            return Err(CoreError::invalid(
                "answers",
                ValidationRule::InvalidParams,
                "a question is answered twice",
            ));
        }
        clean.push(clean_answer(q, a)?);
    }
    let before = get_history(conn, &p.patient_id)?;
    bump(conn, actor, &p.patient_id, p.version, Some(&notes))?;
    conn.execute("DELETE FROM patient_medical_answer WHERE patient_id = ?1", [&p.patient_id])?;
    for a in &clean {
        conn.execute(
            "INSERT INTO patient_medical_answer(patient_id, question_id, answer, detail_text, detail_choice)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![p.patient_id, a.question_id, answer_code(a.answer), a.detail_text, a.detail_choice],
        )?;
    }
    let after = get_history(conn, &p.patient_id)?;
    audit::record(
        conn,
        actor,
        "patient.medical_history_update",
        Some("patient"),
        Some(&p.patient_id),
        Some(&json!({"answers": before.answers, "notes": before.notes})),
        Some(&json!({"answers": after.answers, "notes": after.notes})),
    )?;
    Ok(after)
}

/// "Asked again, nothing changed" (M1: review at each new visit).
pub fn review_history(
    conn: &Connection,
    actor: &Actor,
    p: &ReviewMedicalHistoryParams,
) -> Result<MedicalHistoryInfo> {
    crate::patient::get_patient(conn, &p.patient_id)?;
    bump(conn, actor, &p.patient_id, p.version, None)?;
    let after = get_history(conn, &p.patient_id)?;
    audit::record(
        conn,
        actor,
        "patient.medical_history_review",
        Some("patient"),
        Some(&p.patient_id),
        None,
        None,
    )?;
    Ok(after)
}
