//! Dental specialties (M2 of docs/specs/clinical-workflow-v1.md): reference data of type `specialty`
//! — nine seeded in `core/seeds/reference.csv`, extendable and switchable by the clinic. A doctor has
//! one or more (doctor_specialty); every catalog service belongs to one, so booking or planning a
//! service suggests the doctors of its specialty first.

use std::collections::HashMap;

use artaveo_shared::{ErrorCode, SaveSpecialtyParams, SpecialtyInfo, Translations, ValidationRule};
use rusqlite::{params, Connection};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::clock::now_iso;
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::{new_id, seed_id};
use crate::normalize::normalize_for_search;

pub(crate) fn type_id() -> String {
    seed_id("reference_type", "specialty")
}

/// The id of a seeded specialty by code (`orthodontics`, …).
pub(crate) fn id_of(code: &str) -> String {
    seed_id("reference_item", &format!("specialty/{code}"))
}

fn labels(conn: &Connection) -> Result<HashMap<String, Translations>> {
    let mut stmt = conn.prepare(
        "SELECT t.reference_item_id, t.language_code, t.label FROM reference_translation t
         JOIN reference_item i ON i.id = t.reference_item_id WHERE i.type_id = ?1",
    )?;
    let mut out: HashMap<String, Translations> = HashMap::new();
    for row in stmt.query_map([type_id()], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })? {
        let (id, lang, label) = row?;
        let t = out.entry(id).or_default();
        match lang.as_str() {
            "fa" => t.fa = label,
            "ps" => t.ps = label,
            _ => t.en = label,
        }
    }
    Ok(out)
}

pub fn list(conn: &Connection, include_inactive: bool) -> Result<Vec<SpecialtyInfo>> {
    let all = labels(conn)?;
    let mut stmt = conn.prepare(
        "SELECT id, code, is_system, is_active, sort_order, version FROM reference_item
         WHERE type_id = ?1 AND deleted_at IS NULL AND (?2 = 1 OR is_active = 1) ORDER BY sort_order, code",
    )?;
    let v = stmt
        .query_map(params![type_id(), include_inactive], |r| {
            Ok(SpecialtyInfo {
                id: r.get(0)?,
                code: r.get(1)?,
                is_system: r.get(2)?,
                is_active: r.get(3)?,
                sort_order: r.get(4)?,
                version: r.get(5)?,
                label: Translations::default(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .map(|mut s| {
            s.label = all.get(&s.id).cloned().unwrap_or_default();
            s
        })
        .collect();
    Ok(v)
}

pub fn get(conn: &Connection, id: &str) -> Result<SpecialtyInfo> {
    list(conn, true)?.into_iter().find(|s| s.id == id).ok_or_else(|| {
        CoreError::api(ErrorCode::NotFound, format!("specialty {id}"))
            .on_field("id", ValidationRule::SpecialtyNotFound)
    })
}

/// Every id must be a specialty (active or not: a doctor keeps a specialty the clinic switched off).
pub fn check_ids(conn: &Connection, field: &str, ids: &[String]) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for id in ids.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let ok: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM reference_item WHERE id = ?1 AND type_id = ?2 AND deleted_at IS NULL)",
            params![id, type_id()],
            |r| r.get(0),
        )?;
        if !ok {
            return Err(CoreError::invalid(
                field,
                ValidationRule::SpecialtyNotFound,
                format!("unknown specialty {id}"),
            ));
        }
        if !out.iter().any(|x| x == id) {
            out.push(id.to_string());
        }
    }
    Ok(out)
}

/// The three-language labels of the given specialties (a document's letterhead).
pub fn labels_of(conn: &Connection, ids: &[String]) -> Result<Vec<Translations>> {
    let all = labels(conn)?;
    Ok(ids.iter().filter_map(|id| all.get(id).cloned()).collect())
}

pub fn save(conn: &Connection, actor: &Actor, p: &SaveSpecialtyParams) -> Result<SpecialtyInfo> {
    let now = now_iso();
    let (id, before) = match &p.id {
        None => {
            let label = crate::formulary::clean_translations("label", &p.label, 100)?;
            let id = new_id();
            let sort: i64 = conn.query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM reference_item WHERE type_id = ?1",
                [type_id()],
                |r| r.get(0),
            )?;
            conn.execute(
                "INSERT INTO reference_item(id, type_id, code, sort_order, is_active, is_system, created_at, created_by,
                    updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?6, ?7)",
                params![id, type_id(), format!("custom_{}", id.replace('-', "")), sort, p.is_active, now, actor.user_id],
            )?;
            write_labels(conn, &id, &label)?;
            (id, None)
        }
        Some(id) => {
            let before = get(conn, id)?;
            // A seeded specialty keeps its names (the seed renews them on every start); it can be switched off.
            if !before.is_system {
                let label = crate::formulary::clean_translations("label", &p.label, 100)?;
                write_labels(conn, id, &label)?;
            }
            let changed = conn.execute(
                "UPDATE reference_item SET is_active = ?1, updated_at = ?2, updated_by = ?3, version = version + 1
                 WHERE id = ?4 AND version = ?5 AND deleted_at IS NULL",
                params![p.is_active, now, actor.user_id, id, p.version],
            )?;
            expect_one_row(changed, "specialty")?;
            (id.clone(), Some(before))
        }
    };
    let after = get(conn, &id)?;
    audit::record(
        conn,
        actor,
        if before.is_some() { "specialty.update" } else { "specialty.create" },
        Some("specialty"),
        Some(&id),
        before.as_ref().map(|b| json!(b)).as_ref(),
        Some(&json!(after)),
    )?;
    Ok(after)
}

fn write_labels(conn: &Connection, id: &str, t: &Translations) -> Result<()> {
    for (lang, label) in [("fa", &t.fa), ("ps", &t.ps), ("en", &t.en)] {
        conn.execute(
            "INSERT INTO reference_translation(reference_item_id, language_code, label) VALUES (?1, ?2, ?3)
             ON CONFLICT(reference_item_id, language_code) DO UPDATE SET label = excluded.label",
            params![id, lang, label],
        )?;
    }
    Ok(())
}

/// Words that name each seeded specialty, as typed into the free-text field before v0.5.0.
const ALIASES: &[(&str, &[&str])] = &[
    ("orthodontics", &["ارتودنسی", "ارتودانسی", "ارتودونسی", "ارتوډونسي", "orthodont"]),
    ("endodontics", &["اندو", "عصب کشی", "عصب‌کشی", "endodont", "root canal"]),
    ("oral_surgery", &["جراح", "surgery", "surgeon", "maxillofacial"]),
    ("prosthodontics", &["پروتز", "prosthodont", "prosthetic"]),
    ("periodontics", &["پریو", "لثه", "periodont", "perio"]),
    ("implantology", &["ایمپلنت", "ایمپلانت", "implant"]),
    ("pediatric", &["اطفال", "کودکان", "pediatric", "paediatric", "children"]),
    ("cosmetic", &["زیبایی", "cosmetic", "aesthetic"]),
    ("general", &["عمومی", "general"]),
];

/// The seeded specialties a free-text specialty names (one-time M2 upgrade of existing doctors).
pub(crate) fn match_legacy(text: &str) -> Vec<&'static str> {
    let t = normalize_for_search(text);
    ALIASES
        .iter()
        .filter(|(_, words)| words.iter().any(|w| t.contains(&normalize_for_search(w))))
        .map(|(code, _)| *code)
        .collect()
}

/// Links doctors whose old free-text specialty names a seeded specialty, and clears the text. Runs on
/// every start (after the seed); a doctor already linked, or whose text matches nothing, is left as is.
pub fn link_legacy(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, specialty FROM doctor WHERE specialty IS NOT NULL AND deleted_at IS NULL
         AND NOT EXISTS(SELECT 1 FROM doctor_specialty s WHERE s.doctor_id = doctor.id)",
    )?;
    let rows: Vec<(String, String)> =
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    for (doctor, text) in rows {
        let codes = match_legacy(&text);
        if codes.is_empty() {
            continue;
        }
        for code in codes {
            conn.execute(
                "INSERT OR IGNORE INTO doctor_specialty(doctor_id, specialty_id) VALUES (?1, ?2)",
                params![doctor, id_of(code)],
            )?;
        }
        conn.execute("UPDATE doctor SET specialty = NULL WHERE id = ?1", [&doctor])?;
    }
    Ok(())
}

/// The specialties of every doctor (doctor id → specialty ids, in list order).
pub(crate) fn of_doctors(conn: &Connection) -> Result<HashMap<String, Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT s.doctor_id, s.specialty_id FROM doctor_specialty s JOIN reference_item i ON i.id = s.specialty_id
         ORDER BY i.sort_order, i.code",
    )?;
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (d, s) = row?;
        out.entry(d).or_default().push(s);
    }
    Ok(out)
}

/// Replaces a doctor's specialties.
pub(crate) fn set_for_doctor(conn: &Connection, doctor_id: &str, ids: &[String]) -> Result<()> {
    conn.execute("DELETE FROM doctor_specialty WHERE doctor_id = ?1", [doctor_id])?;
    for id in ids {
        conn.execute(
            "INSERT INTO doctor_specialty(doctor_id, specialty_id) VALUES (?1, ?2)",
            params![doctor_id, id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_free_text_specialties_are_recognised() {
        assert_eq!(match_legacy("ارتودانسی"), vec!["orthodontics"]);
        assert_eq!(match_legacy("Orthodontics"), vec!["orthodontics"]);
        assert_eq!(match_legacy("جراحی و ایمپلنت"), vec!["oral_surgery", "implantology"]);
        assert!(match_legacy("داکتر خوب").is_empty());
    }
}
