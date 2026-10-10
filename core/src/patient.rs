//! Patients & medical records (roadmap Phase 3). Mirrors the shape of
//! `auth.rs`/`clinic.rs`: validate, mutate, re-read, audit. Full-text search
//! (3.4) is maintained explicitly alongside every mutation rather than by SQL
//! triggers, so normalization (`crate::normalize`) stays in one place.

use artaveo_shared::{
    AttachmentInfo, AttachmentKind, CreatePatientParams, MergePatientsParams, PatientInfo, PatientListParams,
    PatientListResult, PatientStatus, UpdatePatientParams, ValidationRule,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use time::macros::format_description;

use crate::audit::{self, Actor};
use crate::calendar::digits::phone_search_key;
use crate::clock::now_iso;
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::new_id;
use crate::normalize::normalize_for_search;

// ───────────────────────────── validation ─────────────────────────────

pub(crate) fn validate_full_name(name: &str) -> Result<()> {
    let len = name.trim().chars().count();
    if !(1..=150).contains(&len) {
        return Err(CoreError::invalid(
            "full_name",
            ValidationRule::FullNameLength,
            "full name must be 1-150 characters",
        ));
    }
    Ok(())
}

pub(crate) fn validate_phone(field: &'static str, phone: &str) -> Result<()> {
    let digits = phone_search_key(phone).len();
    if !(7..=15).contains(&digits) {
        return Err(CoreError::invalid(field, ValidationRule::PhoneFormat, "phone must have 7-15 digits"));
    }
    Ok(())
}

const ISO_DATE: &[time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day]");

pub(crate) fn validate_date(field: &'static str, s: &str) -> Result<()> {
    time::Date::parse(s, &ISO_DATE).map(|_| ()).map_err(|_| {
        CoreError::invalid(field, ValidationRule::DateFormat, format!("`{s}` is not a YYYY-MM-DD date"))
    })
}

/// The subset of patient fields that need validating, borrowed out of
/// whichever params type (`Create`/`UpdatePatientParams` share the same
/// shape on the wire but are distinct Rust structs for the API contract).
#[derive(Clone, Copy)]
struct FieldsToValidate<'a> {
    full_name: &'a str,
    phone: Option<&'a str>,
    secondary_phone: Option<&'a str>,
    date_of_birth: Option<&'a str>,
    approximate_age: Option<i64>,
}

impl<'a> From<&'a CreatePatientParams> for FieldsToValidate<'a> {
    fn from(p: &'a CreatePatientParams) -> Self {
        Self {
            full_name: &p.full_name,
            phone: p.phone.as_deref(),
            secondary_phone: p.secondary_phone.as_deref(),
            date_of_birth: p.date_of_birth.as_deref(),
            approximate_age: p.approximate_age,
        }
    }
}

impl<'a> From<&'a UpdatePatientParams> for FieldsToValidate<'a> {
    fn from(p: &'a UpdatePatientParams) -> Self {
        Self {
            full_name: &p.full_name,
            phone: p.phone.as_deref(),
            secondary_phone: p.secondary_phone.as_deref(),
            date_of_birth: p.date_of_birth.as_deref(),
            approximate_age: p.approximate_age,
        }
    }
}

fn validate_fields<'a>(f: impl Into<FieldsToValidate<'a>>) -> Result<()> {
    let f = f.into();
    validate_full_name(f.full_name)?;
    if let Some(p) = f.phone.filter(|s| !s.is_empty()) {
        validate_phone("phone", p)?;
    }
    if let Some(p) = f.secondary_phone.filter(|s| !s.is_empty()) {
        validate_phone("secondary_phone", p)?;
    }
    if let Some(d) = f.date_of_birth.filter(|s| !s.is_empty()) {
        validate_date("date_of_birth", d)?;
    }
    if let Some(a) = f.approximate_age {
        if !(0..=120).contains(&a) {
            return Err(CoreError::invalid(
                "approximate_age",
                ValidationRule::AgeRange,
                "approximate age must be 0-120",
            ));
        }
    }
    Ok(())
}

// ───────────────────────────── reads ─────────────────────────────

const PATIENT_SELECT: &str =
    "SELECT id, patient_number, full_name, father_name, preferred_language, gender_id,
     date_of_birth,
     CASE WHEN approximate_birth_year IS NOT NULL
          THEN CAST(strftime('%Y', 'now', '+4 hours', '+30 minutes') AS INTEGER) - approximate_birth_year
          ELSE approximate_age END,
     phone, secondary_phone, province_id, district_id, address,
     emergency_contact_name, emergency_contact_phone, emergency_contact_relationship_id, referral_source_id,
     notes, registration_date, status, merged_into_id, version
     FROM patient WHERE deleted_at IS NULL";

fn map_patient(r: &rusqlite::Row<'_>) -> rusqlite::Result<PatientInfo> {
    Ok(PatientInfo {
        id: r.get(0)?,
        patient_number: r.get(1)?,
        full_name: r.get(2)?,
        father_name: r.get(3)?,
        preferred_language: r.get::<_, Option<String>>(4)?.and_then(|s| language_from_code(&s)),
        gender_id: r.get(5)?,
        date_of_birth: r.get(6)?,
        approximate_age: r.get(7)?,
        phone: r.get(8)?,
        secondary_phone: r.get(9)?,
        province_id: r.get(10)?,
        district_id: r.get(11)?,
        address: r.get(12)?,
        emergency_contact_name: r.get(13)?,
        emergency_contact_phone: r.get(14)?,
        emergency_contact_relationship_id: r.get(15)?,
        referral_source_id: r.get(16)?,
        notes: r.get(17)?,
        registration_date: r.get(18)?,
        status: match r.get::<_, String>(19)?.as_str() {
            "inactive" => PatientStatus::Inactive,
            _ => PatientStatus::Active,
        },
        merged_into_id: r.get(20)?,
        version: r.get(21)?,
    })
}

fn language_from_code(s: &str) -> Option<artaveo_shared::Language> {
    match s {
        "fa" => Some(artaveo_shared::Language::Fa),
        "ps" => Some(artaveo_shared::Language::Ps),
        "en" => Some(artaveo_shared::Language::En),
        _ => None,
    }
}

fn language_code(l: artaveo_shared::Language) -> &'static str {
    l.code()
}

fn status_code(s: PatientStatus) -> &'static str {
    match s {
        PatientStatus::Active => "active",
        PatientStatus::Inactive => "inactive",
    }
}

pub fn get_patient(conn: &Connection, id: &str) -> Result<PatientInfo> {
    conn.query_row(&format!("{PATIENT_SELECT} AND id = ?1"), [id], map_patient).optional()?.ok_or_else(|| {
        CoreError::api(artaveo_shared::ErrorCode::NotFound, format!("patient {id}"))
            .on_field("id", ValidationRule::PatientNotFound)
    })
}

/// `query` is matched against the FTS index (name, father's name, patient
/// number, phones — all normalized, 3.4); empty/absent lists everything,
/// newest-registered first, subject to `status`.
pub fn list_patients(conn: &Connection, p: &PatientListParams) -> Result<PatientListResult> {
    let limit = p.limit.clamp(1, 200);
    let status_sql = p.status.map(status_code);
    let query = p.query.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let (items, total) = match query {
        Some(q) => {
            let fts_query = fts_match_query(q);
            if fts_query.is_empty() {
                (Vec::new(), 0)
            } else {
                let mut stmt = conn.prepare(&format!(
                    "{PATIENT_SELECT} AND (?2 IS NULL OR status = ?2)
                     AND id IN (SELECT patient_id FROM patient_fts WHERE patient_fts MATCH ?1)
                     ORDER BY registration_date DESC, patient_number DESC LIMIT ?3 OFFSET ?4"
                ))?;
                let rows: Vec<PatientInfo> = stmt
                    .query_map(params![fts_query, status_sql, limit, p.offset], map_patient)?
                    .collect::<rusqlite::Result<_>>()?;
                // Deleted patients are removed from the index, so without a status filter the
                // index alone counts the matches (no join with 17 000 patient rows).
                let total: u32 = match status_sql {
                    None => conn.query_row(
                        "SELECT COUNT(*) FROM patient_fts WHERE patient_fts MATCH ?1",
                        [&fts_query],
                        |r| r.get(0),
                    )?,
                    Some(_) => conn.query_row(
                        "SELECT COUNT(*) FROM patient WHERE deleted_at IS NULL AND status = ?2
                         AND id IN (SELECT patient_id FROM patient_fts WHERE patient_fts MATCH ?1)",
                        params![fts_query, status_sql],
                        |r| r.get(0),
                    )?,
                };
                (rows, total)
            }
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "{PATIENT_SELECT} AND (?1 IS NULL OR status = ?1)
                 ORDER BY registration_date DESC, patient_number DESC LIMIT ?2 OFFSET ?3"
            ))?;
            let rows: Vec<PatientInfo> = stmt
                .query_map(params![status_sql, limit, p.offset], map_patient)?
                .collect::<rusqlite::Result<_>>()?;
            let total: u32 = conn.query_row(
                "SELECT COUNT(*) FROM patient WHERE deleted_at IS NULL AND (?1 IS NULL OR status = ?1)",
                [status_sql],
                |r| r.get(0),
            )?;
            (rows, total)
        }
    };
    Ok(PatientListResult { items, total })
}

/// Same name, or same phone (3.1): the common real-world duplicate. Reuses
/// the FTS index (built from the same `normalize_for_search`/`phone_search_key`
/// as everything else) rather than re-implementing normalization in SQL.
pub fn check_duplicate(conn: &Connection, full_name: &str, phone: Option<&str>) -> Result<Vec<PatientInfo>> {
    let name_key = normalize_for_search(full_name);
    if name_key.is_empty() {
        return Ok(Vec::new());
    }
    let name_phrase = format!("\"{}\"", name_key.replace('"', ""));
    let phone_key = phone.map(phone_search_key).filter(|s| !s.is_empty());
    let fts_query = match &phone_key {
        Some(p) => format!("{name_phrase} OR \"{p}\""),
        None => name_phrase,
    };
    let mut stmt = conn.prepare(&format!(
        "{PATIENT_SELECT} AND merged_into_id IS NULL
         AND id IN (SELECT patient_id FROM patient_fts WHERE patient_fts MATCH ?1)
         ORDER BY registration_date DESC LIMIT 5"
    ))?;
    let rows: Vec<PatientInfo> =
        stmt.query_map([fts_query], map_patient)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

// ───────────────────────────── search index ─────────────────────────────

fn search_text(
    patient_number: &str,
    full_name: &str,
    father_name: Option<&str>,
    phone: Option<&str>,
    secondary_phone: Option<&str>,
) -> String {
    let mut parts = vec![normalize_for_search(patient_number), normalize_for_search(full_name)];
    if let Some(f) = father_name {
        parts.push(normalize_for_search(f));
    }
    for p in [phone, secondary_phone].into_iter().flatten() {
        let key = phone_search_key(p);
        if !key.is_empty() {
            parts.push(key);
        }
    }
    parts.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

/// A new patient has no index row yet, so there is nothing to delete. (`patient_id` is an
/// UNINDEXED FTS column: deleting by it scans the whole index, which made registering patients
/// quadratic — minutes for 100 000 rows.)
fn fts_insert(conn: &Connection, patient_id: &str, text: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO patient_fts(patient_id, search_text) VALUES (?1, ?2)",
        params![patient_id, text],
    )?;
    Ok(())
}

fn fts_upsert(conn: &Connection, patient_id: &str, text: &str) -> Result<()> {
    conn.execute("DELETE FROM patient_fts WHERE patient_id = ?1", [patient_id])?;
    conn.execute(
        "INSERT INTO patient_fts(patient_id, search_text) VALUES (?1, ?2)",
        params![patient_id, text],
    )?;
    Ok(())
}

fn fts_delete(conn: &Connection, patient_id: &str) -> Result<()> {
    conn.execute("DELETE FROM patient_fts WHERE patient_id = ?1", [patient_id])?;
    Ok(())
}

/// Builds an FTS5 MATCH expression: every normalized token of the query gets
/// a trailing `*` (prefix match) and tokens are ANDed, so typing more of a
/// name or number only narrows the result — the incremental-search UX 3.4 asks for.
fn fts_match_query(q: &str) -> String {
    normalize_for_search(q)
        .split_whitespace()
        .map(|t| format!("\"{}\"*", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}

// ───────────────────────────── writes ─────────────────────────────

fn next_patient_number(conn: &Connection) -> Result<String> {
    let n: i64 = conn.query_row(
        "UPDATE counter SET value = value + 1 WHERE name = 'patient_number' RETURNING value",
        [],
        |r| r.get(0),
    )?;
    Ok(format!("P-{n:06}"))
}

pub fn create_patient(conn: &Connection, actor: &Actor, p: &CreatePatientParams) -> Result<PatientInfo> {
    validate_fields(p)?;
    if !p.allow_duplicate && !check_duplicate(conn, &p.full_name, p.phone.as_deref())?.is_empty() {
        return Err(CoreError::invalid(
            "full_name",
            ValidationRule::PossibleDuplicate,
            "a patient with the same name or phone already exists",
        ));
    }
    let id = new_id();
    let now = now_iso();
    let number = next_patient_number(conn)?;
    let registration_date =
        p.registration_date.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| now[..10].to_string());
    let f = p;
    conn.execute(
        "INSERT INTO patient(
            id, patient_number, full_name, father_name, preferred_language, gender_id, date_of_birth,
            approximate_birth_year, phone, secondary_phone, province_id, district_id, address,
            emergency_contact_name, emergency_contact_phone, emergency_contact_relationship_id,
            referral_source_id, notes, registration_date, status, created_at, created_by, updated_at, updated_by
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,CASE WHEN ?7 IS NULL AND ?8 IS NOT NULL THEN CAST(strftime('%Y', 'now', '+4 hours', '+30 minutes') AS INTEGER) - ?8 END,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,'active',?20,?21,?20,?21)",
        params![
            id,
            number,
            f.full_name.trim(),
            f.father_name.as_deref().map(str::trim),
            f.preferred_language.map(language_code),
            f.gender_id,
            f.date_of_birth.as_deref().filter(|s| !s.is_empty()),
            f.approximate_age,
            f.phone.as_deref().map(str::trim),
            f.secondary_phone.as_deref().map(str::trim),
            f.province_id,
            f.district_id,
            f.address.as_deref().map(str::trim),
            f.emergency_contact_name.as_deref().map(str::trim),
            f.emergency_contact_phone.as_deref().map(str::trim),
            f.emergency_contact_relationship_id,
            f.referral_source_id,
            f.notes.as_deref().map(str::trim),
            registration_date,
            now,
            actor.user_id,
        ],
    )?;
    fts_insert(
        conn,
        &id,
        &search_text(
            &number,
            &f.full_name,
            f.father_name.as_deref(),
            f.phone.as_deref(),
            f.secondary_phone.as_deref(),
        ),
    )?;
    let info = get_patient(conn, &id)?;
    audit::record(conn, actor, "patient.create", Some("patient"), Some(&id), None, Some(&json!(info)))?;
    Ok(info)
}

pub fn update_patient(conn: &Connection, actor: &Actor, p: &UpdatePatientParams) -> Result<PatientInfo> {
    validate_fields(p)?;
    let before = get_patient(conn, &p.id)?;
    let f = p;
    let changed = conn.execute(
        "UPDATE patient SET full_name=?1, father_name=?2, preferred_language=?3, gender_id=?4, date_of_birth=?5,
            approximate_birth_year=CASE WHEN ?5 IS NULL AND ?6 IS NOT NULL THEN CAST(strftime('%Y', 'now', '+4 hours', '+30 minutes') AS INTEGER) - ?6 END, approximate_age=NULL, phone=?7, secondary_phone=?8, province_id=?9, district_id=?10, address=?11,
            emergency_contact_name=?12, emergency_contact_phone=?13, emergency_contact_relationship_id=?14,
            referral_source_id=?15, notes=?16, status=?17, updated_at=?18, updated_by=?19, version=version+1
         WHERE id=?20 AND version=?21 AND deleted_at IS NULL",
        params![
            f.full_name.trim(),
            f.father_name.as_deref().map(str::trim),
            f.preferred_language.map(language_code),
            f.gender_id,
            f.date_of_birth.as_deref().filter(|s| !s.is_empty()),
            f.approximate_age,
            f.phone.as_deref().map(str::trim),
            f.secondary_phone.as_deref().map(str::trim),
            f.province_id,
            f.district_id,
            f.address.as_deref().map(str::trim),
            f.emergency_contact_name.as_deref().map(str::trim),
            f.emergency_contact_phone.as_deref().map(str::trim),
            f.emergency_contact_relationship_id,
            f.referral_source_id,
            f.notes.as_deref().map(str::trim),
            status_code(p.status),
            now_iso(),
            actor.user_id,
            p.id,
            p.version,
        ],
    )?;
    expect_one_row(changed, "patient")?;
    fts_upsert(
        conn,
        &p.id,
        &search_text(
            &before.patient_number,
            &f.full_name,
            f.father_name.as_deref(),
            f.phone.as_deref(),
            f.secondary_phone.as_deref(),
        ),
    )?;
    let after = get_patient(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        "patient.update",
        Some("patient"),
        Some(&p.id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

pub fn delete_patient(conn: &Connection, actor: &Actor, id: &str, version: i64) -> Result<()> {
    let before = get_patient(conn, id)?;
    // A patient who is still booked or waiting must not vanish from the calendar and the queue.
    let open: bool = conn.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM appointment WHERE patient_id = ?1 AND deleted_at IS NULL
                           AND status IN {})",
            crate::scheduling::LIVE_STATUSES_SQL
        ),
        [id],
        |r| r.get(0),
    )?;
    if open {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::PatientHasOpenAppointments,
            "the patient still has open appointments; cancel them first",
        ));
    }
    let changed = conn.execute(
        "UPDATE patient SET deleted_at = ?1, updated_at = ?1, updated_by = ?2, version = version + 1
         WHERE id = ?3 AND version = ?4 AND deleted_at IS NULL",
        params![now_iso(), actor.user_id, id, version],
    )?;
    expect_one_row(changed, "patient")?;
    fts_delete(conn, id)?;
    // Nobody should keep phoning a patient whose record was removed.
    conn.execute(
        "UPDATE recall SET status = 'dismissed', updated_at = ?1, updated_by = ?2, version = version + 1
         WHERE patient_id = ?3 AND status IN ('pending', 'contacted') AND deleted_at IS NULL",
        params![now_iso(), actor.user_id, id],
    )?;
    audit::record(conn, actor, "patient.delete", Some("patient"), Some(id), Some(&json!(before)), None)?;
    Ok(())
}

/// Folds `merge_id` into `keep_id`: the losing record is marked inactive and
/// points at the survivor (3.1). Appointments and recalls (Phase 4) move to the
/// survivor; later phases (invoices, …) must add their own re-parenting here.
pub fn merge_patients(conn: &Connection, actor: &Actor, p: &MergePatientsParams) -> Result<PatientInfo> {
    if p.keep_id == p.merge_id {
        return Err(CoreError::invalid(
            "merge_id",
            ValidationRule::CannotMergeSelf,
            "cannot merge a patient into itself",
        ));
    }
    let keep = get_patient(conn, &p.keep_id)?;
    let merge_before = get_patient(conn, &p.merge_id)?;
    let changed = conn.execute(
        "UPDATE patient SET status = 'inactive', merged_into_id = ?1, updated_at = ?2, updated_by = ?3, version = version + 1
         WHERE id = ?4 AND version = ?5 AND deleted_at IS NULL",
        params![p.keep_id, now_iso(), actor.user_id, p.merge_id, p.merge_id_version],
    )?;
    expect_one_row(changed, "patient")?;
    let moved_appointments = conn.execute(
        "UPDATE appointment SET patient_id = ?1 WHERE patient_id = ?2",
        params![p.keep_id, p.merge_id],
    )?;
    let moved_recalls = conn
        .execute("UPDATE recall SET patient_id = ?1 WHERE patient_id = ?2", params![p.keep_id, p.merge_id])?;
    // Stays searchable (staff may still look the old name up and need to be
    // redirected to `keep`); only `check_duplicate` excludes merged-away
    // records, via `merged_into_id IS NULL` below.
    let merge_after = get_patient(conn, &p.merge_id)?;
    audit::record(
        conn,
        actor,
        "patient.merge",
        Some("patient"),
        Some(&p.merge_id),
        Some(&json!(merge_before)),
        Some(&json!({
            "merged_after": merge_after,
            "kept": keep.id,
            "moved_appointments": moved_appointments,
            "moved_recalls": moved_recalls
        })),
    )?;
    Ok(keep)
}

// ───────────────────────────── attachments (3.5, ADR-12) ─────────────────────────────

fn attachment_kind_code(k: AttachmentKind) -> &'static str {
    match k {
        AttachmentKind::Xray => "xray",
        AttachmentKind::Photo => "photo",
        AttachmentKind::Document => "document",
        AttachmentKind::Scan => "scan",
        AttachmentKind::ConsentForm => "consent_form",
        AttachmentKind::Other => "other",
    }
}

fn attachment_kind_from_code(s: &str) -> AttachmentKind {
    match s {
        "xray" => AttachmentKind::Xray,
        "photo" => AttachmentKind::Photo,
        "document" => AttachmentKind::Document,
        "scan" => AttachmentKind::Scan,
        "consent_form" => AttachmentKind::ConsentForm,
        _ => AttachmentKind::Other,
    }
}

const ATTACHMENT_SELECT: &str =
    "SELECT id, patient_id, kind, file_name, mime_type, size_bytes, tooth, description,
     has_thumbnail, captured_at, created_at, version FROM patient_attachment WHERE deleted_at IS NULL";

fn map_attachment(r: &rusqlite::Row<'_>) -> rusqlite::Result<AttachmentInfo> {
    Ok(AttachmentInfo {
        id: r.get(0)?,
        patient_id: r.get(1)?,
        kind: attachment_kind_from_code(&r.get::<_, String>(2)?),
        file_name: r.get(3)?,
        mime_type: r.get(4)?,
        size_bytes: r.get(5)?,
        tooth: r.get(6)?,
        description: r.get(7)?,
        has_thumbnail: r.get::<_, i64>(8)? == 1,
        captured_at: r.get(9)?,
        created_at: r.get(10)?,
        version: r.get(11)?,
    })
}

pub fn get_attachment(conn: &Connection, id: &str) -> Result<AttachmentInfo> {
    conn.query_row(&format!("{ATTACHMENT_SELECT} AND id = ?1"), [id], map_attachment)
        .optional()?
        .ok_or_else(|| CoreError::api(artaveo_shared::ErrorCode::NotFound, format!("attachment {id}")))
}

pub fn list_attachments(conn: &Connection, patient_id: &str) -> Result<Vec<AttachmentInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{ATTACHMENT_SELECT} AND patient_id = ?1 ORDER BY captured_at DESC, created_at DESC"
    ))?;
    let v = stmt.query_map([patient_id], map_attachment)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

/// Metadata row only; `sha256` is the caller's (`crate::attachment`) proof
/// the bytes are already written to the content-addressed store.
#[allow(clippy::too_many_arguments)]
pub fn create_attachment(
    conn: &Connection,
    actor: &Actor,
    patient_id: &str,
    kind: AttachmentKind,
    file_name: &str,
    mime_type: &str,
    sha256: &str,
    size_bytes: i64,
    tooth: Option<&str>,
    description: Option<&str>,
    thumbnail_sha256: Option<&str>,
    captured_at: &str,
) -> Result<AttachmentInfo> {
    // A patient that doesn't exist (or is soft-deleted) cannot receive attachments.
    get_patient(conn, patient_id)?;
    let id = new_id();
    let now = now_iso();
    conn.execute(
        "INSERT INTO patient_attachment(id, patient_id, kind, file_name, mime_type, sha256, size_bytes, tooth,
            description, has_thumbnail, thumbnail_sha256, captured_at, created_at, created_by, updated_at, updated_by)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?14,?11,?12,?13,?12,?13)",
        params![
            id,
            patient_id,
            attachment_kind_code(kind),
            file_name,
            mime_type,
            sha256,
            size_bytes,
            tooth,
            description,
            thumbnail_sha256.is_some() as i64,
            captured_at,
            now,
            actor.user_id,
            thumbnail_sha256,
        ],
    )?;
    let info = get_attachment(conn, &id)?;
    audit::record(
        conn,
        actor,
        "patient.attachment_add",
        Some("patient"),
        Some(patient_id),
        None,
        Some(&json!(info)),
    )?;
    Ok(info)
}

/// Content hashes of an attachment's file and (if any) its thumbnail.
pub fn attachment_hashes(conn: &Connection, id: &str) -> Result<(String, Option<String>)> {
    conn.query_row(
        "SELECT sha256, thumbnail_sha256 FROM patient_attachment WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()?
    .ok_or_else(|| CoreError::api(artaveo_shared::ErrorCode::NotFound, format!("attachment {id}")))
}

/// Records a thumbnail made after the fact (files saved before v0.4.0). Not a user edit, so the
/// row's version and audit trail are left alone.
pub fn set_attachment_thumbnail(conn: &Connection, id: &str, sha256: &str) -> Result<()> {
    conn.execute(
        "UPDATE patient_attachment SET thumbnail_sha256 = ?1, has_thumbnail = 1 WHERE id = ?2",
        params![sha256, id],
    )?;
    Ok(())
}

pub fn delete_attachment(conn: &Connection, actor: &Actor, id: &str, version: i64) -> Result<AttachmentInfo> {
    let before = get_attachment(conn, id)?;
    let changed = conn.execute(
        "UPDATE patient_attachment SET deleted_at = ?1, updated_at = ?1, updated_by = ?2, version = version + 1
         WHERE id = ?3 AND version = ?4 AND deleted_at IS NULL",
        params![now_iso(), actor.user_id, id, version],
    )?;
    expect_one_row(changed, "attachment")?;
    audit::record(
        conn,
        actor,
        "patient.attachment_delete",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!(before)),
        None,
    )?;
    Ok(before)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_rules() {
        let mut f = FieldsToValidate {
            full_name: "Ahmad Khan",
            phone: None,
            secondary_phone: None,
            date_of_birth: None,
            approximate_age: None,
        };
        assert!(validate_fields(f).is_ok());
        f.full_name = "  ";
        assert!(validate_fields(f).is_err());
        f.full_name = "Ahmad";
        f.phone = Some("123");
        assert!(validate_fields(f).is_err());
        f.phone = Some("0700123456");
        assert!(validate_fields(f).is_ok());
        f.date_of_birth = Some("not-a-date");
        assert!(validate_fields(f).is_err());
        f.date_of_birth = Some("1990-05-20");
        assert!(validate_fields(f).is_ok());
        f.approximate_age = Some(200);
        assert!(validate_fields(f).is_err());
    }

    #[test]
    fn search_text_joins_normalized_fields() {
        let t = search_text("P-000001", "احمد علي", Some("کريم"), Some("0700-123-456"), None);
        assert!(t.contains("0700123456"));
        assert!(t.contains(&normalize_for_search("احمد علي")));
    }

    #[test]
    fn fts_match_query_prefixes_every_token() {
        assert_eq!(fts_match_query("ahmad kh"), "\"ahmad\"* \"kh\"*");
        assert_eq!(fts_match_query(""), "");
    }
}
