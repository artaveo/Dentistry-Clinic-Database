//! The clinic's lists behind printed clinical documents (Phase 5A): the medicine formulary and
//! ready-made prescriptions (5.9), and the editable texts of consent forms and after-treatment
//! instructions (5.10, 5.10b). Built-in entries come from `core/seeds/` — medicines and prescription
//! templates once (then they are the clinic's), document texts until the clinic rewrites them.

use artaveo_shared::{
    DocumentKind, DocumentTemplateInfo, DrugInfo, ErrorCode, Paper, RxItem, RxTemplateInfo, RxTiming,
    SaveDocumentTemplateParams, SaveDrugParams, SaveRxTemplateParams, Translations, ValidationRule,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::json;

use crate::audit::{self, Actor};
use crate::clock::now_iso;
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::{new_id, seed_id};

const DRUGS_PSV: &str = include_str!("../seeds/drugs.psv");
const RX_TEMPLATES_JSON: &str = include_str!("../seeds/rx_templates.json");
const DOCUMENT_TEMPLATES_JSON: &str = include_str!("../seeds/document_templates.json");

/// Dosage forms a prescription line can have (the UI words them).
pub const FORMS: [&str; 12] = [
    "tablet",
    "capsule",
    "syrup",
    "suspension",
    "mouthwash",
    "gel",
    "cream",
    "ointment",
    "injection",
    "drops",
    "spray",
    "other",
];

/// Medicine classes the prescription warnings know (see `documents::rx_warnings`).
pub const CLASSES: [&str; 16] = [
    "penicillin",
    "cephalosporin",
    "nsaid",
    "tetracycline",
    "metronidazole",
    "macrolide",
    "lincosamide",
    "opioid",
    "azole",
    "paracetamol",
    "steroid",
    "local_anesthetic",
    "antiseptic",
    "antifungal",
    "antiviral",
    "ppi",
];

pub(crate) fn timing_code(t: RxTiming) -> &'static str {
    match t {
        RxTiming::AfterFood => "after_food",
        RxTiming::BeforeFood => "before_food",
        RxTiming::WithFood => "with_food",
        RxTiming::Morning => "morning",
        RxTiming::Bedtime => "bedtime",
    }
}

pub(crate) fn timing_from(s: &str) -> Option<RxTiming> {
    match s {
        "after_food" => Some(RxTiming::AfterFood),
        "before_food" => Some(RxTiming::BeforeFood),
        "with_food" => Some(RxTiming::WithFood),
        "morning" => Some(RxTiming::Morning),
        "bedtime" => Some(RxTiming::Bedtime),
        _ => None,
    }
}

pub(crate) fn paper_code(p: Paper) -> &'static str {
    match p {
        Paper::A4 => "a4",
        Paper::A5 => "a5",
        Paper::A6 => "a6",
    }
}

pub(crate) fn paper_from(s: &str) -> Paper {
    match s {
        "a4" => Paper::A4,
        "a6" => Paper::A6,
        _ => Paper::A5,
    }
}

fn opt(s: &str) -> Option<String> {
    (s != "-" && !s.is_empty()).then(|| s.to_string())
}

fn trimmed(s: &Option<String>) -> Option<String> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn too_long(field: &str, s: Option<&str>, max: usize, rule: ValidationRule) -> Result<()> {
    if s.is_some_and(|s| s.chars().count() > max) {
        return Err(CoreError::invalid(field, rule, format!("{field} is at most {max} characters")));
    }
    Ok(())
}

/// The three languages of a clinic-editable label: at least one given; empty ones take the first.
pub(crate) fn clean_translations(field: &str, t: &Translations, max: usize) -> Result<Translations> {
    let parts = [t.fa.trim(), t.ps.trim(), t.en.trim()];
    let Some(first) = parts.iter().find(|p| !p.is_empty()) else {
        return Err(CoreError::invalid(field, ValidationRule::Required, format!("{field} is required")));
    };
    if parts.iter().any(|p| p.chars().count() > max) {
        return Err(CoreError::invalid(
            field,
            ValidationRule::DocumentText,
            format!("{field} is at most {max} characters"),
        ));
    }
    let or = |p: &str| if p.is_empty() { first.to_string() } else { p.to_string() };
    Ok(Translations { fa: or(parts[0]), ps: or(parts[1]), en: or(parts[2]) })
}

// ───────────────────────────── seeds ─────────────────────────────

struct DrugSeed {
    code: String,
    name: String,
    form: String,
    strength: Option<String>,
    classes: Option<String>,
    quantity: Option<String>,
    dose: Option<String>,
    times: Option<i64>,
    timing: Option<String>,
    days: Option<i64>,
    as_needed: bool,
}

fn drug_seeds() -> Result<Vec<DrugSeed>> {
    let mut lines = DRUGS_PSV.lines().filter(|l| !l.trim().is_empty());
    let header = "code|name|form|strength|classes|quantity|dose|times|timing|days|as_needed";
    if lines.next().map(str::trim) != Some(header) {
        return Err(CoreError::validation("drugs.psv header"));
    }
    let bad = |what: String| CoreError::validation(format!("drugs.psv: {what}"));
    lines
        .map(|l| {
            let c: Vec<&str> = l.split('|').map(str::trim).collect();
            if c.len() != 11 {
                return Err(bad(format!("`{l}` needs 11 columns")));
            }
            if !FORMS.contains(&c[2]) {
                return Err(bad(format!("{}: unknown form {}", c[0], c[2])));
            }
            if let Some(classes) = opt(c[4]) {
                if let Some(x) = classes.split(';').find(|x| !CLASSES.contains(x)) {
                    return Err(bad(format!("{}: unknown class {x}", c[0])));
                }
            }
            if opt(c[8]).is_some_and(|t| timing_from(&t).is_none()) {
                return Err(bad(format!("{}: unknown timing {}", c[0], c[8])));
            }
            let num = |s: &str| -> Result<Option<i64>> {
                opt(s)
                    .map(|v| v.parse().map_err(|_| bad(format!("{}: `{v}` is not a number", c[0]))))
                    .transpose()
            };
            Ok(DrugSeed {
                code: c[0].into(),
                name: c[1].into(),
                form: c[2].into(),
                strength: opt(c[3]),
                classes: opt(c[4]),
                quantity: opt(c[5]),
                dose: opt(c[6]),
                times: num(c[7])?,
                timing: opt(c[8]),
                days: num(c[9])?,
                as_needed: c[10] == "1",
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct RxTemplateSeed {
    code: String,
    name: Translations,
    items: Vec<RxTemplateSeedItem>,
}

/// A template line: a formulary medicine by code, with some of its defaults changed.
#[derive(Deserialize)]
struct RxTemplateSeedItem {
    drug: String,
    #[serde(default)]
    days: Option<u16>,
    #[serde(default)]
    quantity: Option<String>,
}

#[derive(Deserialize)]
struct DocumentTemplateSeed {
    kind: DocumentKind,
    code: String,
    paper: Paper,
    title: Translations,
    body: Translations,
}

fn rx_template_seeds() -> Result<Vec<RxTemplateSeed>> {
    serde_json::from_str(RX_TEMPLATES_JSON)
        .map_err(|e| CoreError::validation(format!("rx_templates.json: {e}")))
}

fn document_template_seeds() -> Result<Vec<DocumentTemplateSeed>> {
    let v: Vec<DocumentTemplateSeed> = serde_json::from_str(DOCUMENT_TEMPLATES_JSON)
        .map_err(|e| CoreError::validation(format!("document_templates.json: {e}")))?;
    if let Some(t) = v.iter().find(|t| !matches!(t.kind, DocumentKind::Consent | DocumentKind::PostOp)) {
        return Err(CoreError::validation(format!("document template {}: only consent/post_op", t.code)));
    }
    Ok(v)
}

/// The prescription line a formulary medicine fills in by default.
fn line_of(d: &DrugInfo) -> RxItem {
    RxItem {
        drug_id: Some(d.id.clone()),
        name: format_name(&d.name),
        form: d.form.clone(),
        strength: d.strength.clone(),
        quantity: d.quantity.clone(),
        dose: d.dose.clone(),
        times_per_day: d.times_per_day,
        timing: d.timing,
        days: d.days,
        as_needed: d.as_needed,
        note: None,
    }
}

fn format_name(s: &str) -> String {
    s.trim().to_string()
}

/// Built-in medicines and prescriptions are written once (then they are the clinic's to change);
/// built-in document texts follow the seed until the clinic rewrites them.
pub fn sync_seeds(conn: &Connection) -> Result<()> {
    let now = now_iso();
    for d in drug_seeds()? {
        conn.execute(
            "INSERT INTO drug(id, code, name, form, strength, classes, quantity, dose, times_per_day, timing, days, as_needed,
                is_system, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1, ?13, ?13)
             ON CONFLICT(id) DO NOTHING",
            params![
                seed_id("drug", &d.code),
                d.code,
                d.name,
                d.form,
                d.strength,
                d.classes,
                d.quantity,
                d.dose,
                d.times,
                d.timing,
                d.days,
                d.as_needed,
                now
            ],
        )?;
    }
    for t in rx_template_seeds()? {
        let mut items = Vec::new();
        for i in &t.items {
            let d = get_drug(conn, &seed_id("drug", &i.drug)).map_err(|_| {
                CoreError::validation(format!("rx template {}: unknown drug {}", t.code, i.drug))
            })?;
            let mut line = line_of(&d);
            if i.days.is_some() {
                line.days = i.days;
            }
            if i.quantity.is_some() {
                line.quantity = i.quantity.clone();
            }
            items.push(line);
        }
        conn.execute(
            "INSERT INTO rx_template(id, code, name_fa, name_ps, name_en, items_json, is_system, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?7)
             ON CONFLICT(id) DO NOTHING",
            params![
                seed_id("rx_template", &t.code),
                t.code,
                t.name.fa,
                t.name.ps,
                t.name.en,
                serde_json::to_string(&items)?,
                now
            ],
        )?;
    }
    for t in document_template_seeds()? {
        let id = seed_id("document_template", &t.code);
        let customized: Option<bool> = conn
            .query_row("SELECT customized FROM document_template WHERE id = ?1", [&id], |r| r.get(0))
            .optional()?;
        match customized {
            Some(true) => continue,
            Some(false) => {
                conn.execute(
                    "UPDATE document_template SET paper = ?1 WHERE id = ?2",
                    params![paper_code(t.paper), id],
                )?;
            }
            None => {
                conn.execute(
                    "INSERT INTO document_template(id, kind, code, paper, is_system, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
                    params![id, crate::documents::kind_code(t.kind), t.code, paper_code(t.paper), now],
                )?;
            }
        }
        write_texts(conn, &id, &t.title, &t.body)?;
    }
    Ok(())
}

// ───────────────────────────── drugs ─────────────────────────────

const DRUG_SELECT: &str =
    "SELECT id, code, name, form, strength, classes, quantity, dose, times_per_day, timing, days,
    as_needed, is_system, is_active, version FROM drug WHERE deleted_at IS NULL";

fn map_drug(r: &rusqlite::Row<'_>) -> rusqlite::Result<DrugInfo> {
    let classes: Option<String> = r.get(5)?;
    let timing: Option<String> = r.get(9)?;
    Ok(DrugInfo {
        id: r.get(0)?,
        code: r.get(1)?,
        name: r.get(2)?,
        form: r.get(3)?,
        strength: r.get(4)?,
        classes: classes.map(|c| c.split(';').map(str::to_string).collect()).unwrap_or_default(),
        quantity: r.get(6)?,
        dose: r.get(7)?,
        times_per_day: r.get(8)?,
        timing: timing.as_deref().and_then(timing_from),
        days: r.get(10)?,
        as_needed: r.get(11)?,
        is_system: r.get(12)?,
        is_active: r.get(13)?,
        version: r.get(14)?,
    })
}

pub fn list_drugs(conn: &Connection, include_inactive: bool) -> Result<Vec<DrugInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{DRUG_SELECT} AND (?1 = 1 OR is_active = 1) ORDER BY name COLLATE NOCASE, strength"
    ))?;
    let v = stmt.query_map([include_inactive as i64], map_drug)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn get_drug(conn: &Connection, id: &str) -> Result<DrugInfo> {
    conn.query_row(&format!("{DRUG_SELECT} AND id = ?1"), [id], map_drug).optional()?.ok_or_else(|| {
        CoreError::api(ErrorCode::NotFound, format!("drug {id}")).on_field("id", ValidationRule::DrugNotFound)
    })
}

/// Checks one prescription line (also used for template lines and the formulary's defaults).
pub(crate) fn check_line(prefix: &str, i: &RxItem) -> Result<RxItem> {
    let name = i.name.trim();
    if name.is_empty() {
        return Err(CoreError::invalid(
            &format!("{prefix}.name"),
            ValidationRule::Required,
            "medicine name is required",
        ));
    }
    too_long(&format!("{prefix}.name"), Some(name), 120, ValidationRule::DocumentText)?;
    if !FORMS.contains(&i.form.as_str()) {
        return Err(CoreError::invalid(
            &format!("{prefix}.form"),
            ValidationRule::RxForm,
            "unknown dosage form",
        ));
    }
    let strength = trimmed(&i.strength);
    let quantity = trimmed(&i.quantity);
    let dose = trimmed(&i.dose);
    let note = trimmed(&i.note);
    too_long(&format!("{prefix}.strength"), strength.as_deref(), 40, ValidationRule::DocumentText)?;
    too_long(&format!("{prefix}.quantity"), quantity.as_deref(), 30, ValidationRule::DocumentText)?;
    too_long(&format!("{prefix}.dose"), dose.as_deref(), 30, ValidationRule::DocumentText)?;
    too_long(&format!("{prefix}.note"), note.as_deref(), 200, ValidationRule::DocumentText)?;
    if i.times_per_day.is_some_and(|t| !(1..=12).contains(&t)) {
        return Err(CoreError::invalid(
            &format!("{prefix}.times_per_day"),
            ValidationRule::RxDoseRange,
            "times a day must be 1 to 12",
        ));
    }
    if i.days.is_some_and(|d| !(1..=365).contains(&d)) {
        return Err(CoreError::invalid(
            &format!("{prefix}.days"),
            ValidationRule::RxDoseRange,
            "days must be 1 to 365",
        ));
    }
    Ok(RxItem {
        drug_id: trimmed(&i.drug_id),
        name: name.to_string(),
        form: i.form.clone(),
        strength,
        quantity,
        dose,
        times_per_day: i.times_per_day,
        timing: i.timing,
        days: i.days,
        as_needed: i.as_needed,
        note,
    })
}

pub fn save_drug(conn: &Connection, actor: &Actor, p: &SaveDrugParams) -> Result<DrugInfo> {
    let line = check_line(
        "drug",
        &RxItem {
            drug_id: None,
            name: p.name.clone(),
            form: p.form.clone(),
            strength: p.strength.clone(),
            quantity: p.quantity.clone(),
            dose: p.dose.clone(),
            times_per_day: p.times_per_day,
            timing: p.timing,
            days: p.days,
            as_needed: p.as_needed,
            note: None,
        },
    )
    .map_err(|e| {
        let field = e.field().map(|f| f.trim_start_matches("drug.").to_string());
        match field {
            Some(f) => e.rename_field(&f),
            None => e,
        }
    })?;
    let mut classes: Vec<&str> = p.classes.iter().map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
    classes.dedup();
    if let Some(bad) = classes.iter().find(|c| !CLASSES.contains(c)) {
        return Err(CoreError::invalid(
            "classes",
            ValidationRule::DrugClassUnknown,
            format!("unknown class {bad}"),
        ));
    }
    let classes = (!classes.is_empty()).then(|| classes.join(";"));
    let now = now_iso();
    let timing = line.timing.map(timing_code);
    let (id, before) = match &p.id {
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO drug(id, code, name, form, strength, classes, quantity, dose, times_per_day, timing, days, as_needed,
                    is_active, created_at, created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?14, ?15)",
                params![
                    id,
                    format!("custom_{}", id.replace('-', "")),
                    line.name,
                    line.form,
                    line.strength,
                    classes,
                    line.quantity,
                    line.dose,
                    line.times_per_day,
                    timing,
                    line.days,
                    line.as_needed,
                    p.is_active,
                    now,
                    actor.user_id
                ],
            )?;
            (id, None)
        }
        Some(id) => {
            let before = get_drug(conn, id)?;
            let changed = conn.execute(
                "UPDATE drug SET name=?1, form=?2, strength=?3, classes=?4, quantity=?5, dose=?6, times_per_day=?7, timing=?8,
                    days=?9, as_needed=?10, is_active=?11, updated_at=?12, updated_by=?13, version=version+1
                 WHERE id=?14 AND version=?15 AND deleted_at IS NULL",
                params![
                    line.name,
                    line.form,
                    line.strength,
                    classes,
                    line.quantity,
                    line.dose,
                    line.times_per_day,
                    timing,
                    line.days,
                    line.as_needed,
                    p.is_active,
                    now,
                    actor.user_id,
                    id,
                    p.version
                ],
            )?;
            expect_one_row(changed, "drug")?;
            (id.clone(), Some(before))
        }
    };
    let after = get_drug(conn, &id)?;
    audit::record(
        conn,
        actor,
        if before.is_some() { "drug.update" } else { "drug.create" },
        Some("drug"),
        Some(&id),
        before.as_ref().map(|b| json!(b)).as_ref(),
        Some(&json!(after)),
    )?;
    Ok(after)
}

// ───────────────────────────── prescription templates ─────────────────────────────

const RX_TEMPLATE_SELECT: &str =
    "SELECT id, code, name_fa, name_ps, name_en, items_json, is_system, is_active, version
    FROM rx_template WHERE deleted_at IS NULL";

fn map_rx_template(r: &rusqlite::Row<'_>) -> rusqlite::Result<RxTemplateInfo> {
    let items: String = r.get(5)?;
    Ok(RxTemplateInfo {
        id: r.get(0)?,
        code: r.get(1)?,
        name: Translations { fa: r.get(2)?, ps: r.get(3)?, en: r.get(4)? },
        items: serde_json::from_str(&items).unwrap_or_default(),
        is_system: r.get(6)?,
        is_active: r.get(7)?,
        version: r.get(8)?,
    })
}

pub fn list_rx_templates(conn: &Connection, include_inactive: bool) -> Result<Vec<RxTemplateInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{RX_TEMPLATE_SELECT} AND (?1 = 1 OR is_active = 1) ORDER BY is_system DESC, name_fa COLLATE NOCASE"
    ))?;
    let v = stmt.query_map([include_inactive as i64], map_rx_template)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

fn get_rx_template(conn: &Connection, id: &str) -> Result<RxTemplateInfo> {
    conn.query_row(&format!("{RX_TEMPLATE_SELECT} AND id = ?1"), [id], map_rx_template)
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("rx template {id}"))
                .on_field("id", ValidationRule::TemplateNotFound)
        })
}

pub fn save_rx_template(
    conn: &Connection,
    actor: &Actor,
    p: &SaveRxTemplateParams,
) -> Result<RxTemplateInfo> {
    let name = clean_translations("name", &p.name, 120)?;
    if p.items.is_empty() || p.items.len() > 15 {
        return Err(CoreError::invalid(
            "items",
            ValidationRule::DocumentItems,
            "a template has 1 to 15 medicines",
        ));
    }
    let items = p
        .items
        .iter()
        .enumerate()
        .map(|(i, x)| check_line(&format!("items.{i}"), x))
        .collect::<Result<Vec<_>>>()?;
    let items_json = serde_json::to_string(&items)?;
    let now = now_iso();
    let (id, before) = match &p.id {
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO rx_template(id, code, name_fa, name_ps, name_en, items_json, is_active, created_at, created_by,
                    updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?8, ?9)",
                params![
                    id,
                    format!("custom_{}", id.replace('-', "")),
                    name.fa,
                    name.ps,
                    name.en,
                    items_json,
                    p.is_active,
                    now,
                    actor.user_id
                ],
            )?;
            (id, None)
        }
        Some(id) => {
            let before = get_rx_template(conn, id)?;
            let changed = conn.execute(
                "UPDATE rx_template SET name_fa=?1, name_ps=?2, name_en=?3, items_json=?4, is_active=?5, updated_at=?6,
                    updated_by=?7, version=version+1
                 WHERE id=?8 AND version=?9 AND deleted_at IS NULL",
                params![name.fa, name.ps, name.en, items_json, p.is_active, now, actor.user_id, id, p.version],
            )?;
            expect_one_row(changed, "rx template")?;
            (id.clone(), Some(before))
        }
    };
    let after = get_rx_template(conn, &id)?;
    audit::record(
        conn,
        actor,
        if before.is_some() { "rx_template.update" } else { "rx_template.create" },
        Some("rx_template"),
        Some(&id),
        before.as_ref().map(|b| json!(b)).as_ref(),
        Some(&json!(after)),
    )?;
    Ok(after)
}

// ───────────────────────────── document templates ─────────────────────────────

fn write_texts(conn: &Connection, id: &str, title: &Translations, body: &Translations) -> Result<()> {
    for (lang, t, b) in
        [("fa", &title.fa, &body.fa), ("ps", &title.ps, &body.ps), ("en", &title.en, &body.en)]
    {
        conn.execute(
            "INSERT INTO document_template_text(template_id, language_code, title, body) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(template_id, language_code) DO UPDATE SET title = excluded.title, body = excluded.body",
            params![id, lang, t, b],
        )?;
    }
    Ok(())
}

fn texts(conn: &Connection, id: &str) -> Result<(Translations, Translations)> {
    let mut stmt =
        conn.prepare("SELECT language_code, title, body FROM document_template_text WHERE template_id = ?1")?;
    let mut title = Translations::default();
    let mut body = Translations::default();
    for row in stmt
        .query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
    {
        let (lang, t, b) = row?;
        match lang.as_str() {
            "fa" => (title.fa, body.fa) = (t, b),
            "ps" => (title.ps, body.ps) = (t, b),
            _ => (title.en, body.en) = (t, b),
        }
    }
    Ok((title, body))
}

const TEMPLATE_SELECT: &str = "SELECT id, kind, code, paper, is_system, customized, is_active, version
    FROM document_template WHERE deleted_at IS NULL";

fn map_template(r: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentTemplateInfo> {
    Ok(DocumentTemplateInfo {
        id: r.get(0)?,
        kind: crate::documents::kind_from(&r.get::<_, String>(1)?),
        code: r.get(2)?,
        paper: paper_from(&r.get::<_, String>(3)?),
        is_system: r.get(4)?,
        customized: r.get(5)?,
        is_active: r.get(6)?,
        title: Translations::default(),
        body: Translations::default(),
        version: r.get(7)?,
    })
}

pub fn list_document_templates(
    conn: &Connection,
    kind: Option<DocumentKind>,
    include_inactive: bool,
) -> Result<Vec<DocumentTemplateInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{TEMPLATE_SELECT} AND (?1 IS NULL OR kind = ?1) AND (?2 = 1 OR is_active = 1) ORDER BY kind, is_system DESC, code"
    ))?;
    let rows: Vec<DocumentTemplateInfo> = stmt
        .query_map(params![kind.map(crate::documents::kind_code), include_inactive], map_template)?
        .collect::<rusqlite::Result<_>>()?;
    rows.into_iter()
        .map(|mut t| {
            (t.title, t.body) = texts(conn, &t.id)?;
            Ok(t)
        })
        .collect()
}

pub fn get_document_template(conn: &Connection, id: &str) -> Result<DocumentTemplateInfo> {
    let mut t = conn
        .query_row(&format!("{TEMPLATE_SELECT} AND id = ?1"), [id], map_template)
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("document template {id}"))
                .on_field("id", ValidationRule::TemplateNotFound)
        })?;
    (t.title, t.body) = texts(conn, id)?;
    Ok(t)
}

pub fn save_document_template(
    conn: &Connection,
    actor: &Actor,
    p: &SaveDocumentTemplateParams,
) -> Result<DocumentTemplateInfo> {
    if !matches!(p.kind, DocumentKind::Consent | DocumentKind::PostOp) {
        return Err(CoreError::invalid(
            "kind",
            ValidationRule::TemplateKind,
            "templates are for consent forms and after-treatment instructions",
        ));
    }
    let title = clean_translations("title", &p.title, 200)?;
    let body = clean_translations("body", &p.body, 8000)?;
    let now = now_iso();
    let (id, before) = match &p.id {
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO document_template(id, kind, code, paper, is_active, created_at, created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?6, ?7)",
                params![
                    id,
                    crate::documents::kind_code(p.kind),
                    format!("custom_{}", id.replace('-', "")),
                    paper_code(p.paper),
                    p.is_active,
                    now,
                    actor.user_id
                ],
            )?;
            (id, None)
        }
        Some(id) => {
            let before = get_document_template(conn, id)?;
            if before.kind != p.kind {
                return Err(CoreError::invalid(
                    "kind",
                    ValidationRule::TemplateKind,
                    "a template keeps its kind",
                ));
            }
            // A built-in template whose words change is the clinic's from now on (the seed no longer updates it).
            let customized =
                before.customized || (before.is_system && (before.title != title || before.body != body));
            let changed = conn.execute(
                "UPDATE document_template SET paper=?1, is_active=?2, customized=?3, updated_at=?4, updated_by=?5,
                    version=version+1
                 WHERE id=?6 AND version=?7 AND deleted_at IS NULL",
                params![paper_code(p.paper), p.is_active, customized, now, actor.user_id, id, p.version],
            )?;
            expect_one_row(changed, "document template")?;
            (id.clone(), Some(before))
        }
    };
    write_texts(conn, &id, &title, &body)?;
    let after = get_document_template(conn, &id)?;
    audit::record(
        conn,
        actor,
        if before.is_some() { "document_template.update" } else { "document_template.create" },
        Some("document_template"),
        Some(&id),
        before.as_ref().map(|b| json!(b)).as_ref(),
        Some(&json!(after)),
    )?;
    Ok(after)
}

/// Puts a rewritten built-in template back to the words that ship with the app.
pub fn reset_document_template(
    conn: &Connection,
    actor: &Actor,
    id: &str,
    version: i64,
) -> Result<DocumentTemplateInfo> {
    let before = get_document_template(conn, id)?;
    let seed = document_template_seeds()?
        .into_iter()
        .find(|t| seed_id("document_template", &t.code) == id)
        .ok_or_else(|| {
        CoreError::invalid("id", ValidationRule::TemplateKind, "only built-in templates have a default")
    })?;
    let changed = conn.execute(
        "UPDATE document_template SET paper=?1, customized=0, updated_at=?2, updated_by=?3, version=version+1
         WHERE id=?4 AND version=?5 AND deleted_at IS NULL",
        params![paper_code(seed.paper), now_iso(), actor.user_id, id, version],
    )?;
    expect_one_row(changed, "document template")?;
    write_texts(conn, id, &seed.title, &seed.body)?;
    let after = get_document_template(conn, id)?;
    audit::record(
        conn,
        actor,
        "document_template.reset",
        Some("document_template"),
        Some(id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn seed_files_are_valid() {
        let drugs = drug_seeds().unwrap();
        let codes: HashSet<&str> = drugs.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes.len(), drugs.len(), "drug codes are unique");
        for t in rx_template_seeds().unwrap() {
            for i in &t.items {
                assert!(codes.contains(i.drug.as_str()), "{}: {}", t.code, i.drug);
            }
        }
        let templates = document_template_seeds().unwrap();
        for t in &templates {
            for b in [&t.body.fa, &t.body.ps, &t.body.en] {
                assert!(b.contains("\n") || t.kind == DocumentKind::PostOp, "{}", t.code);
                assert!(!b.trim().is_empty());
            }
            assert!(!t.body.fa.contains(['ي', 'ك']), "{}: Dari uses Persian ی/ک", t.code);
        }
        assert!(templates.iter().filter(|t| t.kind == DocumentKind::Consent).count() >= 5);
        assert!(templates.iter().filter(|t| t.kind == DocumentKind::PostOp).count() >= 5);
    }
}
