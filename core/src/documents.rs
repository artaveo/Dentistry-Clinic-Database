//! Printed clinical documents (Phase 5A, ADR-13): one store, one number series per kind, for
//! prescriptions (5.9), consent forms (5.10) and the other clinical documents (5.10b). A document is
//! kept exactly as issued — patient, doctor and content snapshots — so a reprint is identical and the
//! record shows what the patient was given; a mistake is corrected by voiding it (with a reason) and
//! issuing a new one. How it looks on paper is the UI's shared document frame; this module decides
//! what it says, checks it, numbers it and counts its prints.

use std::collections::{HashMap, HashSet};

use artaveo_shared::{
    AttachDocumentScanParams, CertificateContent, DocumentContent, DocumentDoctor, DocumentInfo,
    DocumentKind, DocumentPatient, DocumentStatus, ErrorCode, ImagingRequestContent, IssueDocumentParams,
    LabOrderContent, Language, PrescriptionContent, RecordPrescription, RecordSnapshot, RecordSummaryContent,
    RecordVisit, ReferralContent, RxItem, RxSeverity, RxWarning, TemplateDocContent, ValidationRule,
    VoidDocumentParams,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::calendar::GregorianDate;
use crate::clock::{local_now, now_iso};
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::formulary::{self, paper_code, paper_from};
use crate::ids::new_id;

/// What can be asked for on an imaging / laboratory request (5.10b).
pub const IMAGING_TESTS: [&str; 11] = [
    "opg",
    "cbct",
    "periapical",
    "bitewing",
    "cephalometric",
    "occlusal",
    "blood_cbc",
    "blood_coagulation",
    "blood_sugar",
    "hepatitis_hiv",
    "other",
];

pub(crate) fn kind_code(k: DocumentKind) -> &'static str {
    match k {
        DocumentKind::Prescription => "prescription",
        DocumentKind::Consent => "consent",
        DocumentKind::PostOp => "post_op",
        DocumentKind::Referral => "referral",
        DocumentKind::ImagingRequest => "imaging_request",
        DocumentKind::Certificate => "certificate",
        DocumentKind::LabOrder => "lab_order",
        DocumentKind::RecordSummary => "record_summary",
    }
}

pub(crate) fn kind_from(s: &str) -> DocumentKind {
    match s {
        "prescription" => DocumentKind::Prescription,
        "consent" => DocumentKind::Consent,
        "post_op" => DocumentKind::PostOp,
        "referral" => DocumentKind::Referral,
        "imaging_request" => DocumentKind::ImagingRequest,
        "lab_order" => DocumentKind::LabOrder,
        "record_summary" => DocumentKind::RecordSummary,
        _ => DocumentKind::Certificate,
    }
}

/// Number series prefix of each kind (`RX-1405-000001`).
fn prefix(k: DocumentKind) -> &'static str {
    match k {
        DocumentKind::Prescription => "RX",
        DocumentKind::Consent => "CF",
        DocumentKind::PostOp => "PO",
        DocumentKind::Referral => "RF",
        DocumentKind::ImagingRequest => "IR",
        DocumentKind::Certificate => "MC",
        DocumentKind::LabOrder => "LO",
        DocumentKind::RecordSummary => "MR",
    }
}

fn content_kind(c: &DocumentContent) -> DocumentKind {
    match c {
        DocumentContent::Prescription(_) => DocumentKind::Prescription,
        DocumentContent::Consent(_) => DocumentKind::Consent,
        DocumentContent::PostOp(_) => DocumentKind::PostOp,
        DocumentContent::Referral(_) => DocumentKind::Referral,
        DocumentContent::ImagingRequest(_) => DocumentKind::ImagingRequest,
        DocumentContent::Certificate(_) => DocumentKind::Certificate,
        DocumentContent::LabOrder(_) => DocumentKind::LabOrder,
        DocumentContent::RecordSummary(_) => DocumentKind::RecordSummary,
    }
}

/// The Shamsi year of today in the clinic (ADR-08): the year in a document number.
fn shamsi_year_today() -> i32 {
    let d = local_now().date();
    GregorianDate { year: d.year(), month: d.month() as u8, day: d.day() }
        .to_shamsi()
        .map(|s| s.year)
        .unwrap_or(0)
}

fn next_number(conn: &Connection, kind: DocumentKind) -> Result<String> {
    let year = shamsi_year_today();
    let p = prefix(kind);
    conn.execute(
        "INSERT INTO document_counter(prefix, year, value) VALUES (?1, ?2, 1)
         ON CONFLICT(prefix, year) DO UPDATE SET value = value + 1",
        params![p, year],
    )?;
    let n: i64 = conn.query_row(
        "SELECT value FROM document_counter WHERE prefix = ?1 AND year = ?2",
        params![p, year],
        |r| r.get(0),
    )?;
    Ok(format!("{p}-{year}-{n:06}"))
}

// ───────────────────────────── snapshots ─────────────────────────────

fn age_on(dob: &str, today: time::Date) -> Option<i64> {
    let d = crate::scheduling::parse_date("date_of_birth", dob).ok()?;
    let mut age = today.year() as i64 - d.year() as i64;
    if (today.month() as u8, today.day()) < (d.month() as u8, d.day()) {
        age -= 1;
    }
    Some(age)
}

fn patient_snapshot(conn: &Connection, patient_id: &str) -> Result<DocumentPatient> {
    let p = crate::patient::get_patient(conn, patient_id)?;
    let gender: Option<String> = match &p.gender_id {
        Some(g) => {
            conn.query_row("SELECT code FROM reference_item WHERE id = ?1", [g], |r| r.get(0)).optional()?
        }
        None => None,
    };
    let age = p.date_of_birth.as_deref().and_then(|d| age_on(d, local_now().date())).or(p.approximate_age);
    Ok(DocumentPatient {
        name: p.full_name,
        number: p.patient_number,
        father_name: p.father_name,
        gender,
        age,
        phone: p.phone,
    })
}

fn doctor_snapshot(conn: &Connection, doctor_id: &str) -> Result<DocumentDoctor> {
    let d = crate::scheduling::get_doctor(conn, doctor_id)?;
    Ok(DocumentDoctor {
        name: d.full_name,
        license_number: d.license_number,
        specialties: crate::specialty::labels_of(conn, &d.specialty_ids)?,
    })
}

// ───────────────────────────── prescription warnings (5.9) ─────────────────────────────

/// Medicine classes guessed from a name typed by hand (not picked from the formulary).
fn guess_classes(name: &str) -> Vec<&'static str> {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    let mut v = Vec::new();
    if has(&["amoxi", "ampicil", "penicil", "cloxa", "augmentin", "co-amox"]) {
        v.push("penicillin");
    }
    if has(&["cefa", "cephal", "cefi", "cefu", "ceftr"]) {
        v.push("cephalosporin");
    }
    if has(&[
        "ibupro", "diclo", "mefena", "naprox", "aspirin", "ketorol", "piroxi", "meloxi", "celecox", "nimesul",
    ]) {
        v.push("nsaid");
    }
    if has(&["doxy", "tetracyc", "minocyc"]) {
        v.push("tetracycline");
    }
    if has(&["metronid", "flagyl", "tinidaz"]) {
        v.push("metronidazole");
    }
    if has(&["azithro", "erythro", "clarithro"]) {
        v.push("macrolide");
    }
    if has(&["tramad", "codein", "morphin"]) {
        v.push("opioid");
    }
    if has(&["fluconaz", "miconaz", "ketoconaz"]) {
        v.push("azole");
    }
    if has(&["paracet", "acetamin"]) {
        v.push("paracetamol");
    }
    if has(&["dexameth", "prednis", "hydrocort"]) {
        v.push("steroid");
    }
    if has(&["lidoca", "lignoca", "benzoca", "articai"]) {
        v.push("local_anesthetic");
    }
    v
}

/// (checklist question, medicine class, rule, severity): a "yes" to the question and a medicine of
/// the class on the prescription raise the rule's warning.
const RULES: &[(&str, &str, &str, RxSeverity)] = &[
    ("allergy_penicillin", "penicillin", "penicillin_allergy", RxSeverity::Danger),
    ("allergy_penicillin", "cephalosporin", "cephalosporin_cross_allergy", RxSeverity::Caution),
    ("allergy_painkillers", "nsaid", "nsaid_allergy", RxSeverity::Danger),
    ("allergy_local_anesthetic", "local_anesthetic", "anesthetic_allergy", RxSeverity::Danger),
    ("pregnant", "nsaid", "nsaid_pregnancy", RxSeverity::Danger),
    ("pregnant", "tetracycline", "tetracycline_pregnancy", RxSeverity::Danger),
    ("anticoagulants", "nsaid", "nsaid_bleeding", RxSeverity::Caution),
    ("bleeding_disorder", "nsaid", "nsaid_bleeding", RxSeverity::Caution),
    ("anticoagulants", "metronidazole", "anticoagulant_interaction", RxSeverity::Caution),
    ("anticoagulants", "azole", "anticoagulant_interaction", RxSeverity::Caution),
    ("anticoagulants", "macrolide", "anticoagulant_interaction", RxSeverity::Caution),
    ("stomach_ulcer", "nsaid", "nsaid_stomach", RxSeverity::Caution),
    ("kidney_disease", "nsaid", "nsaid_kidney", RxSeverity::Caution),
    ("asthma", "nsaid", "nsaid_asthma", RxSeverity::Caution),
    ("hypertension", "nsaid", "nsaid_blood_pressure", RxSeverity::Caution),
    ("breastfeeding", "tetracycline", "breastfeeding_caution", RxSeverity::Caution),
    ("breastfeeding", "opioid", "breastfeeding_caution", RxSeverity::Caution),
    ("breastfeeding", "metronidazole", "breastfeeding_caution", RxSeverity::Caution),
    ("pregnant", "metronidazole", "pregnancy_caution", RxSeverity::Caution),
    ("pregnant", "opioid", "pregnancy_caution", RxSeverity::Caution),
    ("pregnant", "azole", "pregnancy_caution", RxSeverity::Caution),
    ("pregnant", "steroid", "pregnancy_caution", RxSeverity::Caution),
    ("liver_disease", "paracetamol", "liver_dose", RxSeverity::Caution),
    ("liver_disease", "azole", "liver_dose", RxSeverity::Caution),
    ("liver_disease", "metronidazole", "liver_dose", RxSeverity::Caution),
    ("epilepsy", "opioid", "seizure_risk", RxSeverity::Caution),
    ("diabetes", "steroid", "steroid_diabetes", RxSeverity::Caution),
];

/// The warnings for `items` given the patient's "yes" answers (`question code → detail`) and each
/// line's medicine classes. Pure, so every rule is unit-tested without a database.
pub(crate) fn warnings_for(
    items: &[RxItem],
    classes_of: &dyn Fn(&RxItem) -> Vec<String>,
    yes: &HashMap<String, Option<String>>,
) -> Vec<RxWarning> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (i, item) in items.iter().enumerate() {
        let classes = classes_of(item);
        let mut push = |rule: &str, question: &str, severity: RxSeverity| {
            let key = format!("{rule}:{i}");
            if seen.insert(key.clone()) {
                out.push(RxWarning {
                    key,
                    rule: rule.to_string(),
                    item_index: i as u32,
                    question_code: question.to_string(),
                    detail: yes.get(question).cloned().flatten(),
                    severity,
                });
            }
        };
        for (question, class, rule, severity) in RULES {
            if yes.contains_key(*question) && classes.iter().any(|c| c == class) {
                push(rule, question, *severity);
            }
        }
        // "Other medicine allergy: <name>" — the named medicine itself.
        if let Some(Some(named)) = yes.get("allergy_other") {
            let named = named.trim().to_lowercase();
            if named.chars().count() >= 3 && item.name.to_lowercase().contains(&named) {
                push("named_allergy", "allergy_other", RxSeverity::Danger);
            }
        }
    }
    out
}

/// The medicine classes of a prescription line: its formulary entry's, or guessed from its name.
fn classes_lookup(conn: &Connection) -> Result<impl Fn(&RxItem) -> Vec<String>> {
    let drugs: HashMap<String, Vec<String>> =
        formulary::list_drugs(conn, true)?.into_iter().map(|d| (d.id, d.classes)).collect();
    Ok(move |item: &RxItem| {
        let mut c: Vec<String> =
            item.drug_id.as_ref().and_then(|id| drugs.get(id)).cloned().unwrap_or_default();
        for g in guess_classes(&item.name) {
            if !c.iter().any(|x| x == g) {
                c.push(g.to_string());
            }
        }
        c
    })
}

pub fn check_prescription(conn: &Connection, patient_id: &str, items: &[RxItem]) -> Result<Vec<RxWarning>> {
    crate::patient::get_patient(conn, patient_id)?;
    let yes = crate::medical::yes_answers(conn, patient_id)?;
    let classes = classes_lookup(conn)?;
    Ok(warnings_for(items, &classes, &yes))
}

// ───────────────────────────── content checks ─────────────────────────────

fn text(field: &str, s: &str, max: usize) -> Result<String> {
    let s = s.trim();
    if s.is_empty() {
        return Err(CoreError::invalid(field, ValidationRule::Required, format!("{field} is required")));
    }
    if s.chars().count() > max {
        return Err(CoreError::invalid(
            field,
            ValidationRule::DocumentText,
            format!("{field} is at most {max} characters"),
        ));
    }
    Ok(s.to_string())
}

fn opt_text(field: &str, s: &Option<String>, max: usize) -> Result<Option<String>> {
    match s.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(v) => text(field, v, max).map(Some),
    }
}

fn opt_date(field: &'static str, s: &Option<String>) -> Result<Option<String>> {
    match s.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(v) => crate::scheduling::parse_date(field, v).map(|d| Some(crate::scheduling::format_date(d))),
    }
}

fn check_template_doc(conn: &Connection, c: &TemplateDocContent) -> Result<TemplateDocContent> {
    if let Some(t) = c.template_id.as_deref().filter(|t| !t.is_empty()) {
        formulary::get_document_template(conn, t)?;
    }
    Ok(TemplateDocContent {
        template_id: c.template_id.clone().filter(|t| !t.is_empty()),
        title: text("title", &c.title, 200)?,
        body: text("body", &c.body, 8000)?,
        procedure: opt_text("procedure", &c.procedure, 200)?,
        teeth: opt_text("teeth", &c.teeth, 100)?,
    })
}

/// Checks and tidies what a document says; for a prescription also that every warning was accepted.
fn check_content(conn: &Connection, p: &IssueDocumentParams) -> Result<DocumentContent> {
    Ok(match &p.content {
        DocumentContent::Prescription(c) => {
            if c.items.is_empty() || c.items.len() > 15 {
                return Err(CoreError::invalid(
                    "items",
                    ValidationRule::DocumentItems,
                    "a prescription has 1 to 15 medicines",
                ));
            }
            let items = c
                .items
                .iter()
                .enumerate()
                .map(|(i, x)| formulary::check_line(&format!("items.{i}"), x))
                .collect::<Result<Vec<_>>>()?;
            let warnings = check_prescription(conn, &p.patient_id, &items)?;
            if let Some(w) = warnings.iter().find(|w| !c.acknowledged.contains(&w.key)) {
                return Err(CoreError::invalid(
                    &format!("items.{}", w.item_index),
                    ValidationRule::RxWarningNotAcknowledged,
                    format!("warning {} was not acknowledged", w.key),
                ));
            }
            DocumentContent::Prescription(PrescriptionContent {
                items,
                notes: opt_text("notes", &c.notes, 1000)?,
                // Only the warnings that apply are kept with the prescription, as the record of what was accepted.
                acknowledged: warnings.into_iter().map(|w| w.key).collect(),
            })
        }
        DocumentContent::Consent(c) => DocumentContent::Consent(check_template_doc(conn, c)?),
        DocumentContent::PostOp(c) => DocumentContent::PostOp(check_template_doc(conn, c)?),
        DocumentContent::Referral(c) => DocumentContent::Referral(ReferralContent {
            to: text("to", &c.to, 200)?,
            specialty: opt_text("specialty", &c.specialty, 100)?,
            reason: text("reason", &c.reason, 1000)?,
            summary: opt_text("summary", &c.summary, 3000)?,
            urgent: c.urgent,
        }),
        DocumentContent::ImagingRequest(c) => {
            let mut tests: Vec<String> = Vec::new();
            for t in &c.tests {
                if !IMAGING_TESTS.contains(&t.as_str()) {
                    return Err(CoreError::invalid(
                        "tests",
                        ValidationRule::ImagingTestUnknown,
                        format!("unknown test {t}"),
                    ));
                }
                if !tests.contains(t) {
                    tests.push(t.clone());
                }
            }
            if tests.is_empty() {
                return Err(CoreError::invalid(
                    "tests",
                    ValidationRule::DocumentItems,
                    "choose at least one test",
                ));
            }
            let notes = opt_text("notes", &c.notes, 1000)?;
            if tests.iter().any(|t| t == "other") && notes.is_none() {
                return Err(CoreError::invalid("notes", ValidationRule::Required, "say which other test"));
            }
            DocumentContent::ImagingRequest(ImagingRequestContent {
                tests,
                teeth: opt_text("teeth", &c.teeth, 100)?,
                center: opt_text("center", &c.center, 200)?,
                notes,
            })
        }
        DocumentContent::Certificate(c) => {
            let visit_date =
                crate::scheduling::format_date(crate::scheduling::parse_date("visit_date", &c.visit_date)?);
            if c.rest_days.is_some_and(|d| !(1..=60).contains(&d)) {
                return Err(CoreError::invalid(
                    "rest_days",
                    ValidationRule::RestDaysRange,
                    "rest is 1 to 60 days",
                ));
            }
            let rest_from = match c.rest_days {
                Some(_) => opt_date("rest_from", &c.rest_from)?.or_else(|| Some(visit_date.clone())),
                None => None,
            };
            DocumentContent::Certificate(CertificateContent {
                visit_date,
                rest_days: c.rest_days,
                rest_from,
                addressee: opt_text("addressee", &c.addressee, 200)?,
                notes: opt_text("notes", &c.notes, 1000)?,
            })
        }
        DocumentContent::LabOrder(c) => DocumentContent::LabOrder(LabOrderContent {
            lab: opt_text("lab", &c.lab, 200)?,
            teeth: text("teeth", &c.teeth, 100)?,
            work: text("work", &c.work, 200)?,
            material: opt_text("material", &c.material, 100)?,
            shade: opt_text("shade", &c.shade, 20)?,
            due_date: opt_date("due_date", &c.due_date)?,
            notes: opt_text("notes", &c.notes, 1000)?,
        }),
        DocumentContent::RecordSummary(c) => DocumentContent::RecordSummary(RecordSummaryContent {
            purpose: opt_text("purpose", &c.purpose, 200)?,
            snapshot: Some(record_snapshot(conn, &p.patient_id, p.language)?),
        }),
    })
}

/// What a record summary shows, frozen at issue: checklist "yes" answers in the document's language,
/// the notes, and the latest visits and prescriptions.
fn record_snapshot(conn: &Connection, patient_id: &str, lang: Language) -> Result<RecordSnapshot> {
    let history = crate::medical::get_history(conn, patient_id)?;
    let questions: HashMap<String, artaveo_shared::MedicalQuestionInfo> =
        crate::medical::list_questions(conn, true)?.into_iter().map(|q| (q.id.clone(), q)).collect();
    let pick = |t: &artaveo_shared::Translations| match lang {
        Language::Fa => t.fa.clone(),
        Language::Ps => t.ps.clone(),
        Language::En => t.en.clone(),
    };
    let medical = history
        .answers
        .iter()
        .filter(|a| a.answer == artaveo_shared::MedicalAnswerValue::Yes)
        .filter_map(|a| {
            let q = questions.get(&a.question_id)?;
            let text = a.detail_text.as_deref().map(|t| match q.detail_kind {
                artaveo_shared::MedicalDetailKind::Months => crate::medical::months_label(t, lang),
                _ => t.to_string(),
            });
            let choice = a.detail_choice.as_deref().map(|c| crate::medical::choice_label(c, lang));
            let detail = [text, choice].into_iter().flatten().collect::<Vec<_>>();
            let sep = if lang == Language::En { ", " } else { "، " };
            Some(if detail.is_empty() {
                pick(&q.label)
            } else {
                format!("{} ({})", pick(&q.label), detail.join(sep))
            })
        })
        .collect();
    let mut stmt = conn.prepare(
        "SELECT a.local_date, d.full_name, a.status, a.reason FROM appointment a JOIN doctor d ON d.id = a.doctor_id
         WHERE a.patient_id = ?1 AND a.deleted_at IS NULL AND a.status NOT IN ('rescheduled')
         ORDER BY a.start_at DESC LIMIT 15",
    )?;
    let visits = stmt
        .query_map([patient_id], |r| {
            Ok(RecordVisit {
                date: r.get(0)?,
                doctor: r.get(1)?,
                status: artaveo_shared::AppointmentStatus::from_code(&r.get::<_, String>(2)?)
                    .unwrap_or(artaveo_shared::AppointmentStatus::Scheduled),
                reason: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let prescriptions = list(conn, patient_id, Some(DocumentKind::Prescription))?
        .into_iter()
        .filter(|d| d.status == DocumentStatus::Issued)
        .take(10)
        .map(|d| RecordPrescription {
            number: d.number,
            issued_at: d.issued_at,
            medicines: match d.content {
                DocumentContent::Prescription(c) => c.items.into_iter().map(|i| i.name).collect(),
                _ => vec![],
            },
        })
        .collect();
    Ok(RecordSnapshot { medical, medical_notes: history.notes, visits, prescriptions })
}

// ───────────────────────────── store ─────────────────────────────

const SELECT: &str =
    "SELECT d.id, d.number, d.kind, d.patient_id, d.patient_json, d.doctor_id, d.doctor_json,
    d.appointment_id, d.language, d.paper, d.content_json, d.issued_at, u.display_name, d.print_count,
    d.last_printed_at, d.attachment_id, d.status, d.void_reason, d.version
    FROM clinical_document d LEFT JOIN app_user u ON u.id = d.created_by
    WHERE d.deleted_at IS NULL";

fn map(r: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentInfo> {
    let json_err = |i: usize, e: serde_json::Error| {
        rusqlite::Error::FromSqlConversionFailure(i, rusqlite::types::Type::Text, Box::new(e))
    };
    let patient: String = r.get(4)?;
    let doctor: Option<String> = r.get(6)?;
    let content: String = r.get(10)?;
    let language: String = r.get(8)?;
    Ok(DocumentInfo {
        id: r.get(0)?,
        number: r.get(1)?,
        kind: kind_from(&r.get::<_, String>(2)?),
        patient_id: r.get(3)?,
        patient: serde_json::from_str(&patient).map_err(|e| json_err(4, e))?,
        doctor_id: r.get(5)?,
        doctor: doctor.map(|d| serde_json::from_str(&d)).transpose().map_err(|e| json_err(6, e))?,
        appointment_id: r.get(7)?,
        language: match language.as_str() {
            "ps" => Language::Ps,
            "en" => Language::En,
            _ => Language::Fa,
        },
        paper: paper_from(&r.get::<_, String>(9)?),
        content: serde_json::from_str(&content).map_err(|e| json_err(10, e))?,
        issued_at: r.get(11)?,
        issued_by_name: r.get(12)?,
        print_count: r.get(13)?,
        last_printed_at: r.get(14)?,
        attachment_id: r.get(15)?,
        status: if r.get::<_, String>(16)? == "void" { DocumentStatus::Void } else { DocumentStatus::Issued },
        void_reason: r.get(17)?,
        version: r.get(18)?,
    })
}

pub fn get(conn: &Connection, id: &str) -> Result<DocumentInfo> {
    conn.query_row(&format!("{SELECT} AND d.id = ?1"), [id], map).optional()?.ok_or_else(|| {
        CoreError::api(ErrorCode::NotFound, format!("document {id}"))
            .on_field("id", ValidationRule::DocumentNotFound)
    })
}

/// A patient's documents, newest first.
pub fn list(conn: &Connection, patient_id: &str, kind: Option<DocumentKind>) -> Result<Vec<DocumentInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT} AND d.patient_id = ?1 AND (?2 IS NULL OR d.kind = ?2) ORDER BY d.issued_at DESC, d.number DESC"
    ))?;
    let v =
        stmt.query_map(params![patient_id, kind.map(kind_code)], map)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn issue(conn: &Connection, actor: &Actor, p: &IssueDocumentParams) -> Result<DocumentInfo> {
    let patient = patient_snapshot(conn, &p.patient_id)?;
    let kind = content_kind(&p.content);
    let doctor_id = p.doctor_id.as_deref().filter(|d| !d.is_empty());
    if kind == DocumentKind::Prescription && doctor_id.is_none() {
        return Err(CoreError::invalid(
            "doctor_id",
            ValidationRule::DoctorRequired,
            "a prescription needs its doctor",
        ));
    }
    let doctor = doctor_id.map(|d| doctor_snapshot(conn, d)).transpose()?;
    if let Some(a) = p.appointment_id.as_deref().filter(|a| !a.is_empty()) {
        let appt = crate::appointment::get_appointment(conn, a)?;
        if appt.patient_id != p.patient_id {
            return Err(CoreError::invalid(
                "appointment_id",
                ValidationRule::InvalidParams,
                "that visit is another patient's",
            ));
        }
    }
    let content = check_content(conn, p)?;
    let id = new_id();
    let number = next_number(conn, kind)?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO clinical_document(id, number, kind, patient_id, doctor_id, appointment_id, language, paper, patient_json,
            doctor_json, content_json, issued_at, created_at, created_by, updated_at, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, ?13, ?12, ?13)",
        params![
            id,
            number,
            kind_code(kind),
            p.patient_id,
            doctor_id,
            p.appointment_id.as_deref().filter(|a| !a.is_empty()),
            p.language.code(),
            paper_code(p.paper),
            serde_json::to_string(&patient)?,
            doctor.as_ref().map(serde_json::to_string).transpose()?,
            serde_json::to_string(&content)?,
            now,
            actor.user_id
        ],
    )?;
    let doc = get(conn, &id)?;
    audit::record(
        conn,
        actor,
        "document.issue",
        Some("patient"),
        Some(&p.patient_id),
        None,
        Some(&json!({"id": doc.id, "number": doc.number, "kind": kind_code(kind)})),
    )?;
    Ok(doc)
}

fn check_issued(d: &DocumentInfo) -> Result<()> {
    if d.status == DocumentStatus::Void {
        return Err(CoreError::invalid("id", ValidationRule::DocumentVoid, "this document was voided"));
    }
    Ok(())
}

/// A print or a PDF of an issued document (a reprint never makes a new document, 5.9).
pub fn mark_printed(conn: &Connection, actor: &Actor, id: &str, pdf: bool) -> Result<DocumentInfo> {
    let d = get(conn, id)?;
    check_issued(&d)?;
    conn.execute(
        "UPDATE clinical_document SET print_count = print_count + 1, last_printed_at = ?1 WHERE id = ?2",
        params![now_iso(), id],
    )?;
    audit::record(
        conn,
        actor,
        if pdf { "document.pdf" } else { "document.print" },
        Some("patient"),
        Some(&d.patient_id),
        None,
        Some(&json!({"id": d.id, "number": d.number, "copy": d.print_count + 1})),
    )?;
    get(conn, id)
}

pub fn void(conn: &Connection, actor: &Actor, p: &VoidDocumentParams) -> Result<DocumentInfo> {
    let before = get(conn, &p.id)?;
    check_issued(&before)?;
    let reason = text("reason", &p.reason, 300)?;
    let changed = conn.execute(
        "UPDATE clinical_document SET status = 'void', void_reason = ?1, updated_at = ?2, updated_by = ?3, version = version + 1
         WHERE id = ?4 AND version = ?5 AND deleted_at IS NULL",
        params![reason, now_iso(), actor.user_id, p.id, p.version],
    )?;
    expect_one_row(changed, "document")?;
    let after = get(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        "document.void",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!({"id": before.id, "number": before.number})),
        Some(&json!({"reason": after.void_reason})),
    )?;
    Ok(after)
}

/// Links the signed copy of a document (a consent form scanned back in) to it.
pub fn attach_scan(conn: &Connection, actor: &Actor, p: &AttachDocumentScanParams) -> Result<DocumentInfo> {
    let before = get(conn, &p.id)?;
    check_issued(&before)?;
    let a = crate::patient::get_attachment(conn, &p.attachment_id)?;
    if a.patient_id != before.patient_id {
        return Err(CoreError::invalid(
            "attachment_id",
            ValidationRule::InvalidParams,
            "that file is another patient's",
        ));
    }
    let changed = conn.execute(
        "UPDATE clinical_document SET attachment_id = ?1, updated_at = ?2, updated_by = ?3, version = version + 1
         WHERE id = ?4 AND version = ?5 AND deleted_at IS NULL",
        params![p.attachment_id, now_iso(), actor.user_id, p.id, p.version],
    )?;
    expect_one_row(changed, "document")?;
    let after = get(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        "document.attach_scan",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!({"id": before.id, "attachment_id": before.attachment_id})),
        Some(&json!({"attachment_id": after.attachment_id})),
    )?;
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(name: &str) -> RxItem {
        RxItem {
            drug_id: None,
            name: name.into(),
            form: "tablet".into(),
            strength: None,
            quantity: None,
            dose: None,
            times_per_day: None,
            timing: None,
            days: None,
            as_needed: false,
            note: None,
        }
    }

    fn rules(items: &[RxItem], yes: &[(&str, Option<&str>)]) -> Vec<(String, RxSeverity)> {
        let yes: HashMap<String, Option<String>> =
            yes.iter().map(|(k, v)| (k.to_string(), v.map(str::to_string))).collect();
        let classes = |i: &RxItem| guess_classes(&i.name).into_iter().map(str::to_string).collect();
        warnings_for(items, &classes, &yes).into_iter().map(|w| (w.key, w.severity)).collect()
    }

    #[test]
    fn allergies_and_conditions_raise_the_right_warnings() {
        let items = [line("Amoxicillin"), line("Ibuprofen"), line("Metronidazole"), line("Paracetamol")];
        assert_eq!(
            rules(&items, &[("allergy_penicillin", Some("amoxicillin"))]),
            vec![("penicillin_allergy:0".to_string(), RxSeverity::Danger)]
        );
        let w = rules(&items, &[("pregnant", Some("5")), ("anticoagulants", Some("Warfarin"))]);
        assert!(w.contains(&("nsaid_pregnancy:1".into(), RxSeverity::Danger)));
        assert!(w.contains(&("nsaid_bleeding:1".into(), RxSeverity::Caution)));
        assert!(w.contains(&("anticoagulant_interaction:2".into(), RxSeverity::Caution)));
        assert!(w.contains(&("pregnancy_caution:2".into(), RxSeverity::Caution)));
        assert!(rules(&items, &[]).is_empty());
        // "Other medicine allergy: Paracetamol" flags exactly that medicine.
        assert_eq!(
            rules(&items, &[("allergy_other", Some("paracetamol"))]),
            vec![("named_allergy:3".to_string(), RxSeverity::Danger)]
        );
        // The same rule is raised once per line even when two questions lead to it.
        let w = rules(&[line("Diclofenac")], &[("anticoagulants", None), ("bleeding_disorder", None)]);
        assert_eq!(w, vec![("nsaid_bleeding:0".to_string(), RxSeverity::Caution)]);
    }

    #[test]
    fn age_counts_birthdays() {
        let today = time::macros::date!(2026 - 10 - 11);
        assert_eq!(age_on("1990-10-11", today), Some(36));
        assert_eq!(age_on("1990-10-12", today), Some(35));
        assert_eq!(age_on("not a date", today), None);
    }
}
