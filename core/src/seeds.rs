//! System seed data (roadmap 1.5), embedded from `core/seeds/*.csv` and
//! upserted on every start so new releases can add/correct labels in
//! existing clinic databases. Clinic-created reference items are never touched.

use std::collections::HashSet;

use rusqlite::{params, Connection};

use crate::clock::now_iso;
use crate::error::{CoreError, Result};
use crate::ids::seed_id;

const REFERENCE_CSV: &str = include_str!("../seeds/reference.csv");
const PROVINCES_CSV: &str = include_str!("../seeds/provinces.csv");
const DISTRICTS_CSV: &str = include_str!("../seeds/districts.csv");

pub const LANGS: [&str; 3] = ["fa", "ps", "en"];

#[derive(Debug, Clone)]
pub struct ReferenceRow {
    pub type_code: String,
    pub code: String,
    pub sort: i64,
    pub labels: [String; 3],
}

#[derive(Debug, Clone)]
pub struct GeoRow {
    pub parent: Option<String>,
    pub code: String,
    pub sort: i64,
    pub labels: [String; 3],
}

fn rows(csv: &str, expect_header: &str) -> Result<Vec<Vec<String>>> {
    let mut lines = csv.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().unwrap_or_default().trim();
    if header != expect_header {
        return Err(CoreError::validation(format!("seed header `{header}` != `{expect_header}`")));
    }
    let width = expect_header.split(',').count();
    lines
        .enumerate()
        .map(|(i, l)| {
            let cols: Vec<String> = l.split(',').map(|c| c.trim().to_string()).collect();
            if cols.len() != width || cols.iter().any(String::is_empty) {
                return Err(CoreError::validation(format!(
                    "seed line {}: expected {width} non-empty columns",
                    i + 2
                )));
            }
            Ok(cols)
        })
        .collect()
}

fn labels(c: &[String]) -> [String; 3] {
    [c[0].clone(), c[1].clone(), c[2].clone()]
}

fn parse_sort(s: &str) -> Result<i64> {
    s.parse().map_err(|_| CoreError::validation(format!("seed sort `{s}` is not a number")))
}

pub fn reference_rows() -> Result<Vec<ReferenceRow>> {
    let rows = rows(REFERENCE_CSV, "type,code,sort,fa,ps,en")?;
    let mut seen = HashSet::new();
    rows.into_iter()
        .map(|c| {
            if !seen.insert((c[0].clone(), c[1].clone())) {
                return Err(CoreError::validation(format!("duplicate reference {}/{}", c[0], c[1])));
            }
            Ok(ReferenceRow {
                type_code: c[0].clone(),
                code: c[1].clone(),
                sort: parse_sort(&c[2])?,
                labels: labels(&c[3..]),
            })
        })
        .collect()
}

pub fn province_rows() -> Result<Vec<GeoRow>> {
    let rows = rows(PROVINCES_CSV, "code,sort,fa,ps,en")?;
    let mut seen = HashSet::new();
    rows.into_iter()
        .map(|c| {
            if !seen.insert(c[0].clone()) {
                return Err(CoreError::validation(format!("duplicate province {}", c[0])));
            }
            Ok(GeoRow { parent: None, code: c[0].clone(), sort: parse_sort(&c[1])?, labels: labels(&c[2..]) })
        })
        .collect()
}

pub fn district_rows() -> Result<Vec<GeoRow>> {
    let provinces: HashSet<String> = province_rows()?.into_iter().map(|p| p.code).collect();
    let rows = rows(DISTRICTS_CSV, "province_code,code,sort,fa,ps,en")?;
    let mut seen = HashSet::new();
    rows.into_iter()
        .map(|c| {
            if !provinces.contains(&c[0]) {
                return Err(CoreError::validation(format!(
                    "district {} has unknown province {}",
                    c[1], c[0]
                )));
            }
            if !seen.insert(c[1].clone()) {
                return Err(CoreError::validation(format!("duplicate district {}", c[1])));
            }
            Ok(GeoRow {
                parent: Some(c[0].clone()),
                code: c[1].clone(),
                sort: parse_sort(&c[2])?,
                labels: labels(&c[3..]),
            })
        })
        .collect()
}

/// Idempotent upsert of all system seed data, in one transaction.
pub fn sync(conn: &mut Connection) -> Result<()> {
    let now = now_iso();
    let tx = conn.transaction()?;

    for r in reference_rows()? {
        let type_id = seed_id("reference_type", &r.type_code);
        tx.execute(
            "INSERT INTO reference_type(id, code) VALUES (?1, ?2) ON CONFLICT(id) DO NOTHING",
            params![type_id, r.type_code],
        )?;
        let item_id = seed_id("reference_item", &format!("{}/{}", r.type_code, r.code));
        tx.execute(
            "INSERT INTO reference_item(id, type_id, code, sort_order, is_system, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET sort_order = excluded.sort_order",
            params![item_id, type_id, r.code, r.sort, now],
        )?;
        for (lang, label) in LANGS.iter().zip(&r.labels) {
            tx.execute(
                "INSERT INTO reference_translation(reference_item_id, language_code, label) VALUES (?1, ?2, ?3)
                 ON CONFLICT(reference_item_id, language_code) DO UPDATE SET label = excluded.label",
                params![item_id, lang, label],
            )?;
        }
    }

    let geo = |entity: &str, id: &str, labels: &[String; 3]| -> Result<()> {
        for (lang, label) in LANGS.iter().zip(labels) {
            tx.execute(
                "INSERT INTO geo_translation(entity_type, entity_id, language_code, label) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(entity_type, entity_id, language_code) DO UPDATE SET label = excluded.label",
                params![entity, id, lang, label],
            )?;
        }
        Ok(())
    };
    for p in province_rows()? {
        let id = seed_id("province", &p.code);
        tx.execute(
            "INSERT INTO province(id, code, sort_order) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET sort_order = excluded.sort_order",
            params![id, p.code, p.sort],
        )?;
        geo("province", &id, &p.labels)?;
    }
    for d in district_rows()? {
        let id = seed_id("district", &d.code);
        let province_id = seed_id("province", d.parent.as_deref().unwrap());
        tx.execute(
            "INSERT INTO district(id, province_id, code, sort_order) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET sort_order = excluded.sort_order",
            params![id, province_id, d.code, d.sort],
        )?;
        geo("district", &id, &d.labels)?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_files_are_valid() {
        let refs = reference_rows().unwrap();
        for t in ["gender", "blood_group", "marital_status", "payment_method"] {
            assert!(refs.iter().any(|r| r.type_code == t), "{t}");
        }
        let provinces = province_rows().unwrap();
        assert_eq!(provinces.len(), 34, "Afghanistan has 34 provinces");
        assert!(provinces.iter().all(|p| p.code.starts_with("AF-") && p.code.len() == 6));
        let districts = district_rows().unwrap();
        assert_eq!(districts.len(), 404);
        for p in &provinces {
            let centre = format!("{}-CENTER", p.code);
            assert!(districts.iter().any(|d| d.code == centre), "{} has no provincial centre", p.code);
        }
        // Dari labels use Persian ی/ک, never Arabic ي/ك (search and sorting depend on it).
        for d in &districts {
            assert!(!d.labels[0].contains(['ي', 'ك']), "{}: {}", d.code, d.labels[0]);
        }
    }
}
