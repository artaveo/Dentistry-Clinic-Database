//! Finance settings (receipt footer, the clinic's legal profile) and doctor shares: the rules and the
//! preview of what each doctor earned in a period, and the statements that freeze it (DS).

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};

use artaveo_shared::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::clock::now_iso;
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::new_id;

use super::util::*;

// ───────────────────────────── settings ─────────────────────────────

pub fn get_settings(conn: &Connection) -> Result<FinanceSettings> {
    Ok(FinanceSettings {
        share_enabled: setting(conn, "fin.share_enabled")?.as_deref() == Some("1"),
        share_basis: match setting(conn, "fin.share_basis")?.as_deref() {
            Some("treatment") => CommissionBasis::Treatment,
            _ => CommissionBasis::Payment,
        },
        receipt_footer: setting(conn, "fin.receipt_footer")?.unwrap_or_default(),
        legal: setting(conn, "fin.legal")?.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default(),
    })
}

fn clean_legal(l: &LegalProfile) -> Result<LegalProfile> {
    let tin = optional("tin", &l.tin, 20)?;
    if tin.as_deref().is_some_and(|t| !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
        return Err(CoreError::invalid(
            "tin",
            ValidationRule::InvalidParams,
            "a TIN has digits and letters only",
        ));
    }
    Ok(LegalProfile {
        legal_name: optional("legal_name", &l.legal_name, 120)?,
        legal_form: l.legal_form,
        tax_regime: l.tax_regime,
        tin,
        business_license_no: optional("business_license_no", &l.business_license_no, 40)?,
        health_license_no: optional("health_license_no", &l.health_license_no, 40)?,
        tax_office: optional("tax_office", &l.tax_office, 120)?,
        owner_name: optional("owner_name", &l.owner_name, 100)?,
        owner_father_name: optional("owner_father_name", &l.owner_father_name, 100)?,
        official_address: optional("official_address", &l.official_address, 300)?,
    })
}

pub fn save_settings(conn: &Connection, actor: &Actor, p: &FinanceSettings) -> Result<FinanceSettings> {
    let before = get_settings(conn)?;
    let footer = p.receipt_footer.trim();
    if footer.chars().count() > 200 {
        return Err(CoreError::invalid(
            "receipt_footer",
            ValidationRule::DocumentText,
            "at most 200 characters",
        ));
    }
    let legal = clean_legal(&p.legal)?;
    set_setting(conn, actor, "fin.share_enabled", if p.share_enabled { "1" } else { "0" })?;
    set_setting(
        conn,
        actor,
        "fin.share_basis",
        match p.share_basis {
            CommissionBasis::Payment => "payment",
            CommissionBasis::Treatment => "treatment",
        },
    )?;
    set_setting(conn, actor, "fin.receipt_footer", footer)?;
    set_setting(conn, actor, "fin.legal", &serde_json::to_string(&legal)?)?;
    let after = get_settings(conn)?;
    audit::record(
        conn,
        actor,
        "billing.settings",
        Some("setting"),
        None,
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

// ───────────────────────────── rules ─────────────────────────────

fn kind_from(s: &str) -> CommissionKind {
    if s == "fixed" {
        CommissionKind::Fixed
    } else {
        CommissionKind::Percent
    }
}

pub fn list_rules(conn: &Connection) -> Result<Vec<CommissionRuleInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, doctor_id, service_id, kind, value, version FROM commission_rule WHERE deleted_at IS NULL
         ORDER BY doctor_id IS NOT NULL, doctor_id, service_id IS NOT NULL, service_id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CommissionRuleInfo {
                id: r.get(0)?,
                doctor_id: r.get(1)?,
                service_id: r.get(2)?,
                kind: kind_from(&r.get::<_, String>(3)?),
                value: r.get(4)?,
                version: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn save_rule(
    conn: &Connection,
    actor: &Actor,
    p: &SaveCommissionRuleParams,
) -> Result<CommissionRuleInfo> {
    let doctor = check_doctor(conn, &p.doctor_id)?;
    let service = match p.service_id.as_deref().filter(|s| !s.is_empty()) {
        None => None,
        Some(s) => {
            let found: Option<String> = conn
                .query_row("SELECT id FROM service WHERE id = ?1 AND deleted_at IS NULL", [s], |r| r.get(0))
                .optional()?;
            Some(found.ok_or_else(|| {
                CoreError::invalid("service_id", ValidationRule::ServiceNotFound, "unknown service")
            })?)
        }
    };
    let ok = match p.kind {
        CommissionKind::Percent => (0..=10_000).contains(&p.value),
        CommissionKind::Fixed => (0..=MAX_AMOUNT).contains(&p.value),
    };
    if !ok {
        return Err(CoreError::invalid(
            "value",
            ValidationRule::CommissionValue,
            "the share value is out of range",
        ));
    }
    let kind = if p.kind == CommissionKind::Fixed { "fixed" } else { "percent" };
    let now = now_iso();
    // One rule per doctor/service pair: saving the same pair again changes that rule.
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM commission_rule
             WHERE COALESCE(doctor_id, '') = COALESCE(?1, '') AND COALESCE(service_id, '') = COALESCE(?2, '') AND deleted_at IS NULL",
            params![doctor, service],
            |r| r.get(0),
        )
        .optional()?;
    let id = match (p.id.as_deref().filter(|s| !s.is_empty()), existing) {
        (Some(id), _) => {
            let changed = conn.execute(
                "UPDATE commission_rule SET doctor_id = ?1, service_id = ?2, kind = ?3, value = ?4, updated_at = ?5, updated_by = ?6,
                    version = version + 1 WHERE id = ?7 AND version = ?8 AND deleted_at IS NULL",
                params![doctor, service, kind, p.value, now, actor.user_id, id, p.version.unwrap_or(1)],
            )?;
            expect_one_row(changed, "commission rule")?;
            id.to_string()
        }
        (None, Some(id)) => {
            conn.execute(
                "UPDATE commission_rule SET kind = ?1, value = ?2, updated_at = ?3, updated_by = ?4, version = version + 1 WHERE id = ?5",
                params![kind, p.value, now, actor.user_id, id],
            )?;
            id
        }
        (None, None) => {
            let id = new_id();
            conn.execute(
                "INSERT INTO commission_rule(id, doctor_id, service_id, kind, value, created_at, created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?6, ?7)",
                params![id, doctor, service, kind, p.value, now, actor.user_id],
            )?;
            id
        }
    };
    let after = list_rules(conn)?.into_iter().find(|r| r.id == id).ok_or_else(|| {
        CoreError::api(ErrorCode::NotFound, "commission rule")
            .on_field("id", ValidationRule::CommissionRuleNotFound)
    })?;
    audit::record(
        conn,
        actor,
        "commission.rule",
        Some("commission_rule"),
        Some(&id),
        None,
        Some(&json!(after)),
    )?;
    Ok(after)
}

pub fn delete_rule(conn: &Connection, actor: &Actor, id: &str, version: i64) -> Result<()> {
    let changed = conn.execute(
        "UPDATE commission_rule SET deleted_at = ?1, updated_at = ?1, updated_by = ?2, version = version + 1
         WHERE id = ?3 AND version = ?4 AND deleted_at IS NULL",
        params![now_iso(), actor.user_id, id, version],
    )?;
    expect_one_row(changed, "commission rule")?;
    audit::record(conn, actor, "commission.rule_delete", Some("commission_rule"), Some(id), None, None)?;
    Ok(())
}

/// The most specific rule: doctor + service, doctor, service, everyone.
fn rate<'a>(
    rules: &'a [CommissionRuleInfo],
    doctor: &str,
    service: Option<&str>,
) -> Option<&'a CommissionRuleInfo> {
    let find = |d: Option<&str>, s: Option<&str>| {
        rules.iter().find(|r| r.doctor_id.as_deref() == d && r.service_id.as_deref() == s)
    };
    find(Some(doctor), service)
        .filter(|_| service.is_some())
        .or_else(|| find(Some(doctor), None))
        .or_else(|| find(None, service).filter(|_| service.is_some()))
        .or_else(|| find(None, None))
}

fn share_of(rule: Option<&CommissionRuleInfo>, base: i64, quantity: i64) -> i64 {
    match rule {
        None => 0,
        Some(r) => match r.kind {
            CommissionKind::Percent => ((base as i128) * (r.value as i128) / 10_000) as i64,
            CommissionKind::Fixed => r.value * quantity,
        },
    }
}

/// `a × b / c` without overflow (0 when c is 0).
fn muldiv(a: i64, b: i64, c: i64) -> i64 {
    if c == 0 {
        0
    } else {
        ((a as i128) * (b as i128) / (c as i128)) as i64
    }
}

/// `(invoice id, date, invoice number, patient, signed money for the payment basis)`.
type Event = (i64, i64, String, String, Option<i64>);

fn invoice_events(conn: &Connection, from: i64, to: i64) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT d.id, d.doc_date, d.year, d.serial, p.full_name FROM fin_doc d JOIN patient p ON p.id = d.party_id
         WHERE d.kind = 'inv' AND d.status = 'posted' AND d.doc_date BETWEEN ?1 AND ?2 ORDER BY d.id",
    )?;
    let rows = stmt
        .query_map(params![from, to], |r| {
            Ok((r.get(0)?, r.get(1)?, format_number("INV", r.get(2)?, r.get(3)?), r.get(4)?, None))
        })?
        .collect::<rusqlite::Result<Vec<Event>>>()?;
    Ok(rows)
}

fn payment_events(conn: &Connection, from: i64, to: i64) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, a.doc_date, i.year, i.serial, p.full_name, l.amount
         FROM fin_line l JOIN fin_doc a ON a.id = l.doc_id JOIN fin_doc i ON i.id = l.ref_id
            JOIN patient p ON p.id = i.party_id
         WHERE a.kind IN ('rc', 'al') AND a.status = 'posted' AND i.status = 'posted' AND a.doc_date BETWEEN ?1 AND ?2
         ORDER BY a.id, l.no",
    )?;
    let rows = stmt
        .query_map(params![from, to], |r| {
            Ok((r.get(0)?, r.get(1)?, format_number("INV", r.get(2)?, r.get(3)?), r.get(4)?, Some(r.get(5)?)))
        })?
        .collect::<rusqlite::Result<Vec<Event>>>()?;
    Ok(rows)
}

/// What each doctor earned in a period under the clinic's rules (not frozen: a preview).
///
/// Treatment basis: each line of the invoices of the period, its net after the invoice's later
/// reductions and write-offs. Payment basis: each allocation in the period (negative when taken
/// back) shared over the invoice's lines by what they earn.
pub fn preview(conn: &Connection, p: &CommissionReportParams) -> Result<CommissionReport> {
    let (from, to) = period(&p.from, &p.to)?;
    let settings = get_settings(conn)?;
    let mut report = CommissionReport {
        basis: settings.share_basis,
        from: iso_of(from),
        to: iso_of(to),
        doctors: Vec::new(),
        base_total: 0,
        commission_total: 0,
    };
    if !settings.share_enabled {
        return Ok(report);
    }
    let rules = list_rules(conn)?;
    let doctor_filter = p.doctor_id.as_deref().filter(|s| !s.is_empty());
    let events = match settings.share_basis {
        CommissionBasis::Treatment => invoice_events(conn, from, to)?,
        CommissionBasis::Payment => payment_events(conn, from, to)?,
    };
    let ids: Vec<i64> = events.iter().map(|e| e.0).collect();
    let lines = super::invoices::line_nets(conn, &ids)?;
    let mut by_doctor: BTreeMap<String, Vec<CommissionLine>> = BTreeMap::new();
    let mut names: HashMap<String, String> = HashMap::new();
    for (invoice, date, number, patient, money) in events {
        let s = super::invoices::settled(conn, invoice)?;
        // What the invoice really earns after later reductions and write-offs.
        let earned_total = s.net - s.reductions - s.written_off;
        if earned_total <= 0 {
            continue;
        }
        for (provider, service, name, qty, line_net) in lines.get(&invoice).into_iter().flatten() {
            let Some(doctor) = provider.as_deref() else { continue };
            if doctor_filter.is_some_and(|d| d != doctor) {
                continue;
            }
            let earned = muldiv(*line_net, earned_total, s.net);
            let rule = rate(&rules, doctor, service.as_deref());
            let (base, commission) = match money {
                None => (earned, share_of(rule, earned, *qty)),
                Some(m) => {
                    let base = muldiv(earned, m, earned_total);
                    let commission = match rule.map(|r| r.kind) {
                        Some(CommissionKind::Fixed) => muldiv(share_of(rule, 0, *qty), m, earned_total),
                        _ => share_of(rule, base, *qty),
                    };
                    (base, commission)
                }
            };
            if let Entry::Vacant(e) = names.entry(doctor.to_string()) {
                e.insert(crate::scheduling::get_doctor(conn, doctor)?.full_name);
            }
            by_doctor.entry(doctor.to_string()).or_default().push(CommissionLine {
                invoice_id: invoice,
                service_id: service.clone(),
                date: iso_of(date),
                invoice_number: number.clone(),
                patient_name: patient.clone(),
                name: name.clone(),
                base,
                commission,
            });
        }
    }
    for (doctor_id, lines) in by_doctor {
        let base: i64 = lines.iter().map(|l| l.base).sum();
        let commission: i64 = lines.iter().map(|l| l.commission).sum();
        report.base_total += base;
        report.commission_total += commission;
        report.doctors.push(DoctorCommission {
            doctor_name: names.get(&doctor_id).cloned().unwrap_or_default(),
            doctor_id,
            base,
            commission,
            lines,
        });
    }
    report.doctors.sort_by(|a, b| a.doctor_name.cmp(&b.doctor_name));
    Ok(report)
}

// ───────────────────────────── frozen statements (DS) ─────────────────────────────

/// `ext_id` of a statement or payroll: its period as `yyyymmdd-yyyymmdd`.
pub(crate) fn period_key(from: i64, to: i64) -> String {
    format!("{from}-{to}")
}

pub(crate) fn period_of(key: &Option<String>) -> (i64, i64) {
    let k = key.as_deref().unwrap_or("");
    let f = k.get(0..8).and_then(|x| x.parse().ok()).unwrap_or(0);
    let t = k.get(9..17).and_then(|x| x.parse().ok()).unwrap_or(0);
    (f, t)
}

/// SQL: a posted document of kind ?4 (for party ?1 when given) whose period overlaps ?2..?3.
pub(crate) const OVERLAP_SQL: &str =
    "SELECT EXISTS(SELECT 1 FROM fin_doc WHERE kind = ?4 AND status = 'posted'
    AND (?1 IS NULL OR party_id = ?1)
    AND CAST(substr(ext_id, 1, 8) AS INTEGER) <= ?3 AND CAST(substr(ext_id, 10, 8) AS INTEGER) >= ?2)";

/// Approves what a doctor earned in a period: the preview's lines are copied into the statement and
/// never recomputed (a later price reduction does not change it). Books Dr 5120 / Cr 2320 (the doctor).
pub fn create_statement(
    conn: &Connection,
    actor: &Actor,
    p: &CreateShareStatementParams,
) -> Result<ShareStatementInfo> {
    use super::ledger::{cr, dr, insert_doc, post, NewDoc, DOCTOR_SHARES, SHARES_PAYABLE};
    let doctor = check_doctor(conn, &Some(p.doctor_id.clone()))?
        .ok_or_else(|| CoreError::invalid("doctor_id", ValidationRule::Required, "choose the doctor"))?;
    let (from, to) = period(&p.from, &p.to)?;
    let date = doc_day("date", &p.date)?;
    let taken: bool = conn.query_row(OVERLAP_SQL, params![doctor, from, to, "ds"], |r| r.get(0))?;
    if taken {
        return Err(CoreError::invalid(
            "from",
            ValidationRule::PeriodTaken,
            "an approved statement of this doctor already covers part of this period",
        ));
    }
    let settings = get_settings(conn)?;
    let report = preview(
        conn,
        &CommissionReportParams { doctor_id: Some(doctor.clone()), from: iso_of(from), to: iso_of(to) },
    )?;
    let lines: Vec<CommissionLine> = report.doctors.into_iter().flat_map(|d| d.lines).collect();
    let share: i64 = lines.iter().map(|l| l.commission).sum();
    if share <= 0 {
        return Err(CoreError::invalid(
            "doctor_id",
            ValidationRule::BillingAmount,
            "this doctor has no share in this period",
        ));
    }
    let key = period_key(from, to);
    let (id, number) = insert_doc(
        conn,
        actor,
        &NewDoc {
            kind: Some(DocKind::Ds),
            date,
            doctor_id: Some(&doctor),
            amount: share,
            ext_id: Some(&key),
            purpose: Some(match settings.share_basis {
                CommissionBasis::Treatment => "treatment",
                CommissionBasis::Payment => "payment",
            }),
            ..Default::default()
        },
    )?;
    for (i, l) in lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO fin_line(doc_id, no, amount, ref_id, list_price, service_id, provider_id, name, due_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                i as i64 + 1,
                l.commission,
                l.invoice_id,
                l.base,
                l.service_id,
                doctor,
                l.name,
                ymd(&l.date)
            ],
        )?;
    }
    post(conn, id, date, vec![dr(DOCTOR_SHARES, share), cr(SHARES_PAYABLE, share).of(&doctor)])?;
    audit::record(
        conn,
        actor,
        "share.approve",
        Some("doctor"),
        Some(&doctor),
        None,
        Some(&json!({"number": number, "from": iso_of(from), "to": iso_of(to), "share": share})),
    )?;
    get_statement(conn, id)
}

/// `(id, year, serial, doctor, doctor name, period, date, basis, share, status, void reason, prints, by)`
type StatementRow = (
    i64,
    i64,
    i64,
    String,
    String,
    Option<String>,
    i64,
    Option<String>,
    i64,
    String,
    Option<String>,
    i64,
    Option<String>,
);

pub fn get_statement(conn: &Connection, id: i64) -> Result<ShareStatementInfo> {
    let row: StatementRow = conn
        .query_row(
            "SELECT d.id, d.year, d.serial, d.party_id, COALESCE(dr.full_name, ''), d.ext_id, d.doc_date, d.purpose,
                d.amount, d.status, d.void_reason, d.print_count, d.created_by
             FROM fin_doc d LEFT JOIN doctor dr ON dr.id = d.party_id WHERE d.id = ?1 AND d.kind = 'ds'",
            [id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("share statement {id}"))
                .on_field("id", ValidationRule::DocNotFound)
        })?;
    let (
        id,
        year,
        serial,
        doctor_id,
        doctor_name,
        key,
        date,
        basis,
        share,
        status,
        void_reason,
        print_count,
        by,
    ) = row;
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT l.ref_id, l.service_id, l.due_date, {}, COALESCE(p.full_name, ''), COALESCE(l.name, ''),
            COALESCE(l.list_price, 0), l.amount
         FROM fin_line l LEFT JOIN fin_doc d ON d.id = l.ref_id LEFT JOIN patient p ON p.id = d.party_id
         WHERE l.doc_id = ?1 ORDER BY l.no",
        super::ledger::NUMBER_SQL
    ))?;
    let lines: Vec<CommissionLine> = stmt
        .query_map([id], |r| {
            Ok(CommissionLine {
                invoice_id: r.get::<_, Option<i64>>(0)?.unwrap_or(0),
                service_id: r.get(1)?,
                date: iso_of(r.get::<_, Option<i64>>(2)?.unwrap_or(0)),
                invoice_number: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                patient_name: r.get(4)?,
                name: r.get(5)?,
                base: r.get(6)?,
                commission: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let (from, to) = period_of(&key);
    Ok(ShareStatementInfo {
        id,
        number: format_number("DS", year, serial),
        doctor_id,
        doctor_name,
        from: iso_of(from),
        to: iso_of(to),
        date: iso_of(date),
        basis: if basis.as_deref() == Some("treatment") {
            CommissionBasis::Treatment
        } else {
            CommissionBasis::Payment
        },
        base: lines.iter().map(|l| l.base).sum(),
        share,
        lines,
        status: super::ledger::status_of(&status),
        void_reason,
        print_count,
        created_by_name: user_name(conn, &by)?,
    })
}

pub fn list_statements(conn: &Connection, p: &ShareStatementListParams) -> Result<Vec<ShareStatementInfo>> {
    let doctor = p.doctor_id.as_deref().filter(|s| !s.is_empty());
    let mut stmt = conn.prepare(
        "SELECT id FROM fin_doc WHERE kind = 'ds' AND (?1 IS NULL OR party_id = ?1)
         ORDER BY doc_date DESC, id DESC LIMIT 500",
    )?;
    let ids: Vec<i64> = stmt.query_map([doctor], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    ids.into_iter().map(|id| get_statement(conn, id)).collect()
}

/// Per doctor: shares approved (net of voids), paid, and still owed (the 2320 sub-ledger).
pub fn balances(conn: &Connection) -> Result<Vec<DoctorShareBalance>> {
    let mut stmt = conn.prepare(
        "SELECT g.party_id, COALESCE(dr.full_name, ''),
            -COALESCE(SUM(CASE WHEN d.kind = 'ds' THEN g.amount END), 0),
            COALESCE(SUM(CASE WHEN d.kind = 'pv' THEN g.amount END), 0),
            -SUM(g.amount)
         FROM fin_gl g JOIN fin_doc d ON d.id = g.doc_id LEFT JOIN doctor dr ON dr.id = g.party_id
         WHERE g.account_id = 2320 AND g.party_id IS NOT NULL
         GROUP BY g.party_id ORDER BY 2",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(DoctorShareBalance {
                doctor_id: r.get(0)?,
                doctor_name: r.get(1)?,
                approved: r.get(2)?,
                paid: r.get(3)?,
                owed: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(
        doctor: Option<&str>,
        service: Option<&str>,
        kind: CommissionKind,
        value: i64,
    ) -> CommissionRuleInfo {
        CommissionRuleInfo {
            id: format!("{doctor:?}{service:?}"),
            doctor_id: doctor.map(Into::into),
            service_id: service.map(Into::into),
            kind,
            value,
            version: 1,
        }
    }

    #[test]
    fn the_most_specific_rule_wins() {
        let rules = vec![
            rule(None, None, CommissionKind::Percent, 1000),
            rule(Some("d1"), None, CommissionKind::Percent, 2000),
            rule(None, Some("s1"), CommissionKind::Percent, 3000),
            rule(Some("d1"), Some("s1"), CommissionKind::Fixed, 5000),
        ];
        assert_eq!(rate(&rules, "d1", Some("s1")).unwrap().value, 5000);
        assert_eq!(rate(&rules, "d1", Some("s2")).unwrap().value, 2000);
        assert_eq!(rate(&rules, "d2", Some("s1")).unwrap().value, 3000);
        assert_eq!(rate(&rules, "d2", Some("s2")).unwrap().value, 1000);
        assert!(rate(&[], "d1", None).is_none());
    }

    #[test]
    fn a_share_is_a_percent_of_the_base_or_a_fixed_sum_per_unit() {
        let pct = rule(None, None, CommissionKind::Percent, 2500);
        assert_eq!(share_of(Some(&pct), 400_000, 1), 100_000);
        let fixed = rule(None, None, CommissionKind::Fixed, 20_000);
        assert_eq!(share_of(Some(&fixed), 400_000, 3), 60_000);
        assert_eq!(share_of(None, 400_000, 1), 0);
        assert_eq!(muldiv(500_000, 900_000, 1_000_000), 450_000);
        assert_eq!(muldiv(1, 1, 0), 0);
    }
}
