//! Service catalog (5.1 + M3 of docs/specs/clinical-workflow-v1.md): category → service → variant.
//! Seeded once from `core/seeds/services.json` (with the doctor's approval of the spec's table), then
//! the clinic's own: codes, three-language names, prices (AFN × 100, ADR-06), specialty (M2), tooth
//! scope, surface, usual number of visits, lab work, and the consent form and after-treatment sheet
//! each service suggests. Treatment plans and treatments (5B) and invoices (Phase 6) use these rows.

use artaveo_shared::{
    CatalogInfo, DocumentKind, ErrorCode, SaveServiceCategoryParams, SaveServiceParams, ServiceCategoryInfo,
    ServiceInfo, ToothScope, Translations, ValidationRule,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::json;

use crate::audit::{self, Actor};
use crate::clock::now_iso;
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::formulary::clean_translations;
use crate::ids::{new_id, seed_id};

const SERVICES_JSON: &str = include_str!("../seeds/services.json");

/// Highest price accepted: 100 000 000 AFN (in AFN × 100).
const MAX_PRICE: i64 = 10_000_000_000;

pub(crate) fn scope_code(s: ToothScope) -> &'static str {
    match s {
        ToothScope::Tooth => "tooth",
        ToothScope::Teeth => "teeth",
        ToothScope::Quadrant => "quadrant",
        ToothScope::Arch => "arch",
        ToothScope::Mouth => "mouth",
        ToothScope::None => "none",
    }
}

fn scope_from(s: &str) -> ToothScope {
    match s {
        "tooth" => ToothScope::Tooth,
        "teeth" => ToothScope::Teeth,
        "quadrant" => ToothScope::Quadrant,
        "arch" => ToothScope::Arch,
        "mouth" => ToothScope::Mouth,
        _ => ToothScope::None,
    }
}

// ───────────────────────────── seed ─────────────────────────────

#[derive(Deserialize)]
struct CategorySeed {
    code: String,
    specialty: String,
    name: Translations,
    services: Vec<ServiceSeed>,
}

#[derive(Deserialize)]
struct ServiceSeed {
    code: String,
    name: Translations,
    #[serde(default)]
    scope: Option<ToothScope>,
    #[serde(default)]
    surface: Option<bool>,
    #[serde(default)]
    sessions: Option<i64>,
    #[serde(default)]
    lab: Option<bool>,
    #[serde(default)]
    consent: Option<String>,
    #[serde(default)]
    post_op: Option<String>,
    #[serde(default)]
    variants: Vec<ServiceSeed>,
}

fn seeds() -> Result<Vec<CategorySeed>> {
    serde_json::from_str(SERVICES_JSON).map_err(|e| CoreError::validation(format!("services.json: {e}")))
}

/// Writes the seeded catalog once; afterwards every row is the clinic's (prices, names, codes).
pub fn sync_seeds(conn: &Connection) -> Result<()> {
    let now = now_iso();
    for (ci, c) in seeds()?.iter().enumerate() {
        let cat_id = seed_id("service_category", &c.code);
        conn.execute(
            "INSERT INTO service_category(id, code, name_fa, name_ps, name_en, specialty_id, sort_order, is_system,
                created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8)
             ON CONFLICT(id) DO NOTHING",
            params![
                cat_id,
                c.code,
                c.name.fa,
                c.name.ps,
                c.name.en,
                crate::specialty::id_of(&c.specialty),
                (ci as i64 + 1) * 10,
                now
            ],
        )?;
        for (si, s) in c.services.iter().enumerate() {
            let sid = insert_seed(conn, &cat_id, None, s, None, (si as i64 + 1) * 10, &now)?;
            for (vi, v) in s.variants.iter().enumerate() {
                insert_seed(conn, &cat_id, Some(&sid), v, Some(s), (vi as i64 + 1) * 10, &now)?;
            }
        }
    }
    Ok(())
}

fn insert_seed(
    conn: &Connection,
    category_id: &str,
    parent_id: Option<&str>,
    s: &ServiceSeed,
    parent: Option<&ServiceSeed>,
    sort: i64,
    now: &str,
) -> Result<String> {
    let id = seed_id("service", &s.code);
    // A variant takes its service's settings unless it says otherwise.
    let pick = |own: Option<bool>, up: Option<bool>| own.or(up).unwrap_or(false);
    let scope = s.scope.or(parent.and_then(|p| p.scope)).unwrap_or(ToothScope::None);
    let template = |code: Option<&String>| code.map(|c| seed_id("document_template", c));
    conn.execute(
        "INSERT INTO service(id, code, category_id, parent_id, name_fa, name_ps, name_en, price, tooth_scope, needs_surface,
            sessions, lab_required, consent_template_id, post_op_template_id, sort_order, is_system, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 1, ?15, ?15)
         ON CONFLICT(id) DO NOTHING",
        params![
            id,
            s.code,
            category_id,
            parent_id,
            s.name.fa,
            s.name.ps,
            s.name.en,
            scope_code(scope),
            pick(s.surface, parent.and_then(|p| p.surface)),
            s.sessions.or(parent.and_then(|p| p.sessions)).unwrap_or(1),
            pick(s.lab, parent.and_then(|p| p.lab)),
            template(s.consent.as_ref().or(parent.and_then(|p| p.consent.as_ref()))),
            template(s.post_op.as_ref().or(parent.and_then(|p| p.post_op.as_ref()))),
            sort,
            now
        ],
    )?;
    Ok(id)
}

// ───────────────────────────── reads ─────────────────────────────

const CATEGORY_SELECT: &str =
    "SELECT id, code, name_fa, name_ps, name_en, specialty_id, sort_order, is_system, is_active,
    version FROM service_category WHERE deleted_at IS NULL";

fn map_category(r: &rusqlite::Row<'_>) -> rusqlite::Result<ServiceCategoryInfo> {
    Ok(ServiceCategoryInfo {
        id: r.get(0)?,
        code: r.get(1)?,
        name: Translations { fa: r.get(2)?, ps: r.get(3)?, en: r.get(4)? },
        specialty_id: r.get(5)?,
        sort_order: r.get(6)?,
        is_system: r.get(7)?,
        is_active: r.get(8)?,
        version: r.get(9)?,
    })
}

/// The service's specialty is its own, else its parent service's, else its category's (M2).
const SERVICE_SELECT: &str = "SELECT s.id, s.code, s.category_id, s.parent_id, s.name_fa, s.name_ps, s.name_en, s.price,
    COALESCE(s.specialty_id, p.specialty_id, c.specialty_id), s.tooth_scope, s.needs_surface, s.sessions, s.lab_required,
    s.consent_template_id, s.post_op_template_id, s.sort_order, s.is_system, s.is_active, s.version
    FROM service s
    JOIN service_category c ON c.id = s.category_id
    LEFT JOIN service p ON p.id = s.parent_id
    WHERE s.deleted_at IS NULL";

fn map_service(r: &rusqlite::Row<'_>) -> rusqlite::Result<ServiceInfo> {
    Ok(ServiceInfo {
        id: r.get(0)?,
        code: r.get(1)?,
        category_id: r.get(2)?,
        parent_id: r.get(3)?,
        name: Translations { fa: r.get(4)?, ps: r.get(5)?, en: r.get(6)? },
        price: r.get(7)?,
        specialty_id: r.get(8)?,
        tooth_scope: scope_from(&r.get::<_, String>(9)?),
        needs_surface: r.get(10)?,
        sessions: r.get(11)?,
        lab_required: r.get(12)?,
        consent_template_id: r.get(13)?,
        post_op_template_id: r.get(14)?,
        sort_order: r.get(15)?,
        is_system: r.get(16)?,
        is_active: r.get(17)?,
        version: r.get(18)?,
    })
}

pub fn get(conn: &Connection, include_inactive: bool) -> Result<CatalogInfo> {
    let mut stmt =
        conn.prepare(&format!("{CATEGORY_SELECT} AND (?1 = 1 OR is_active = 1) ORDER BY sort_order, code"))?;
    let categories = stmt.query_map([include_inactive], map_category)?.collect::<rusqlite::Result<_>>()?;
    // A switched-off service hides its variants too; services in category order, each followed by its variants.
    let mut stmt = conn.prepare(&format!(
        "{SERVICE_SELECT} AND (?1 = 1 OR (s.is_active = 1 AND c.is_active = 1 AND (p.id IS NULL OR p.is_active = 1)))
         ORDER BY c.sort_order, COALESCE(p.sort_order, s.sort_order), COALESCE(p.code, s.code),
                  s.parent_id IS NOT NULL, s.sort_order, s.code"
    ))?;
    let services = stmt.query_map([include_inactive], map_service)?.collect::<rusqlite::Result<_>>()?;
    Ok(CatalogInfo { categories, services })
}

pub fn get_category(conn: &Connection, id: &str) -> Result<ServiceCategoryInfo> {
    conn.query_row(&format!("{CATEGORY_SELECT} AND id = ?1"), [id], map_category).optional()?.ok_or_else(
        || {
            CoreError::api(ErrorCode::NotFound, format!("service category {id}"))
                .on_field("category_id", ValidationRule::ServiceCategoryNotFound)
        },
    )
}

pub fn get_service(conn: &Connection, id: &str) -> Result<ServiceInfo> {
    conn.query_row(&format!("{SERVICE_SELECT} AND s.id = ?1"), [id], map_service).optional()?.ok_or_else(
        || {
            CoreError::api(ErrorCode::NotFound, format!("service {id}"))
                .on_field("id", ValidationRule::ServiceNotFound)
        },
    )
}

// ───────────────────────────── writes ─────────────────────────────

fn check_specialty(conn: &Connection, id: &Option<String>) -> Result<Option<String>> {
    match id.as_deref().filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => Ok(crate::specialty::check_ids(conn, "specialty_id", &[s.to_string()])?.pop()),
    }
}

pub fn save_category(
    conn: &Connection,
    actor: &Actor,
    p: &SaveServiceCategoryParams,
) -> Result<ServiceCategoryInfo> {
    let name = clean_translations("name", &p.name, 120)?;
    let specialty = check_specialty(conn, &p.specialty_id)?;
    let now = now_iso();
    let (id, before) = match &p.id {
        None => {
            let id = new_id();
            let sort: i64 =
                conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 10 FROM service_category", [], |r| {
                    r.get(0)
                })?;
            conn.execute(
                "INSERT INTO service_category(id, code, name_fa, name_ps, name_en, specialty_id, sort_order, is_active,
                    created_at, created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?9, ?10)",
                params![
                    id,
                    format!("custom_{}", id.replace('-', "")),
                    name.fa,
                    name.ps,
                    name.en,
                    specialty,
                    sort,
                    p.is_active,
                    now,
                    actor.user_id
                ],
            )?;
            (id, None)
        }
        Some(id) => {
            let before = get_category(conn, id)?;
            let changed = conn.execute(
                "UPDATE service_category SET name_fa=?1, name_ps=?2, name_en=?3, specialty_id=?4, is_active=?5,
                    updated_at=?6, updated_by=?7, version=version+1
                 WHERE id=?8 AND version=?9 AND deleted_at IS NULL",
                params![name.fa, name.ps, name.en, specialty, p.is_active, now, actor.user_id, id, p.version],
            )?;
            expect_one_row(changed, "service category")?;
            (id.clone(), Some(before))
        }
    };
    let after = get_category(conn, &id)?;
    audit::record(
        conn,
        actor,
        if before.is_some() { "service_category.update" } else { "service_category.create" },
        Some("service_category"),
        Some(&id),
        before.as_ref().map(|b| json!(b)).as_ref(),
        Some(&json!(after)),
    )?;
    Ok(after)
}

fn check_template(
    conn: &Connection,
    field: &str,
    id: &Option<String>,
    kind: DocumentKind,
) -> Result<Option<String>> {
    let Some(t) = id.as_deref().filter(|s| !s.is_empty()) else { return Ok(None) };
    let template = crate::formulary::get_document_template(conn, t).map_err(|e| e.rename_field(field))?;
    if template.kind != kind {
        return Err(CoreError::invalid(field, ValidationRule::TemplateKind, "a template of another kind"));
    }
    Ok(Some(t.to_string()))
}

pub fn save_service(conn: &Connection, actor: &Actor, p: &SaveServiceParams) -> Result<ServiceInfo> {
    let code = p.code.trim().to_string();
    let len = code.chars().count();
    if !(1..=20).contains(&len) || !code.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        return Err(CoreError::invalid(
            "code",
            ValidationRule::ServiceCodeFormat,
            "a code is 1–20 Latin letters, digits, - _ .",
        ));
    }
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM service WHERE code = ?1 COLLATE NOCASE AND deleted_at IS NULL AND id IS NOT ?2)",
        params![code, p.id],
        |r| r.get(0),
    )?;
    if taken {
        return Err(CoreError::invalid(
            "code",
            ValidationRule::ServiceCodeTaken,
            "another service has this code",
        ));
    }
    let name = clean_translations("name", &p.name, 120)?;
    if !(0..=MAX_PRICE).contains(&p.price) {
        return Err(CoreError::invalid("price", ValidationRule::ServicePrice, "the price is out of range"));
    }
    if !(1..=60).contains(&p.sessions) {
        return Err(CoreError::invalid("sessions", ValidationRule::ServiceSessions, "1 to 60 visits"));
    }
    let mut category_id = get_category(conn, &p.category_id)?.id;
    let parent = match p.parent_id.as_deref().filter(|s| !s.is_empty()) {
        None => None,
        Some(pid) => {
            let parent = get_service(conn, pid).map_err(|e| e.rename_field("parent_id"))?;
            if parent.parent_id.is_some() || p.id.as_deref() == Some(pid) {
                return Err(CoreError::invalid(
                    "parent_id",
                    ValidationRule::ServiceVariantDepth,
                    "a variant belongs to a service, not to another variant",
                ));
            }
            // A variant lives in its service's category.
            category_id = parent.category_id.clone();
            Some(parent.id)
        }
    };
    if let (Some(id), Some(_)) = (&p.id, &parent) {
        let has_variants: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM service WHERE parent_id = ?1 AND deleted_at IS NULL)",
            [id],
            |r| r.get(0),
        )?;
        if has_variants {
            return Err(CoreError::invalid(
                "parent_id",
                ValidationRule::ServiceVariantDepth,
                "a service with variants cannot become a variant",
            ));
        }
    }
    let specialty = check_specialty(conn, &p.specialty_id)?;
    let consent = check_template(conn, "consent_template_id", &p.consent_template_id, DocumentKind::Consent)?;
    let post_op = check_template(conn, "post_op_template_id", &p.post_op_template_id, DocumentKind::PostOp)?;
    let now = now_iso();
    let (id, before) = match &p.id {
        None => {
            let id = new_id();
            let sort: i64 = conn.query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 10 FROM service WHERE category_id = ?1 AND parent_id IS ?2",
                params![category_id, parent],
                |r| r.get(0),
            )?;
            conn.execute(
                "INSERT INTO service(id, code, category_id, parent_id, name_fa, name_ps, name_en, price, specialty_id,
                    tooth_scope, needs_surface, sessions, lab_required, consent_template_id, post_op_template_id,
                    sort_order, is_active, created_at, created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?18, ?19)",
                params![
                    id,
                    code,
                    category_id,
                    parent,
                    name.fa,
                    name.ps,
                    name.en,
                    p.price,
                    specialty,
                    scope_code(p.tooth_scope),
                    p.needs_surface,
                    p.sessions,
                    p.lab_required,
                    consent,
                    post_op,
                    sort,
                    p.is_active,
                    now,
                    actor.user_id
                ],
            )?;
            (id, None)
        }
        Some(id) => {
            let before = get_service(conn, id)?;
            let changed = conn.execute(
                "UPDATE service SET code=?1, category_id=?2, parent_id=?3, name_fa=?4, name_ps=?5, name_en=?6, price=?7,
                    specialty_id=?8, tooth_scope=?9, needs_surface=?10, sessions=?11, lab_required=?12,
                    consent_template_id=?13, post_op_template_id=?14, is_active=?15, updated_at=?16, updated_by=?17,
                    version=version+1
                 WHERE id=?18 AND version=?19 AND deleted_at IS NULL",
                params![
                    code,
                    category_id,
                    parent,
                    name.fa,
                    name.ps,
                    name.en,
                    p.price,
                    specialty,
                    scope_code(p.tooth_scope),
                    p.needs_surface,
                    p.sessions,
                    p.lab_required,
                    consent,
                    post_op,
                    p.is_active,
                    now,
                    actor.user_id,
                    id,
                    p.version
                ],
            )?;
            expect_one_row(changed, "service")?;
            // Variants follow their service into another category.
            conn.execute(
                "UPDATE service SET category_id = ?1 WHERE parent_id = ?2",
                params![category_id, id],
            )?;
            (id.clone(), Some(before))
        }
    };
    let after = get_service(conn, &id)?;
    audit::record(
        conn,
        actor,
        if before.is_some() { "service.update" } else { "service.create" },
        Some("service"),
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
    fn seed_matches_the_spec_table() {
        let cats = seeds().unwrap();
        assert_eq!(cats.len(), 11, "M3 lists 11 categories");
        let mut codes = std::collections::HashSet::new();
        for c in &cats {
            for s in c.services.iter().chain(c.services.iter().flat_map(|s| s.variants.iter())) {
                assert!(codes.insert(s.code.clone()), "duplicate code {}", s.code);
                assert!(!s.name.fa.contains(['ي', 'ك']), "{}", s.code);
            }
        }
        let rct = cats.iter().flat_map(|c| &c.services).find(|s| s.code == "END-01").unwrap();
        assert_eq!(rct.variants.len(), 3);
        assert_eq!(rct.consent.as_deref(), Some("consent_root_canal"));
    }
}
