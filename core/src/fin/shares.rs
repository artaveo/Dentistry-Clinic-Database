//! Finance settings (receipt footer, the clinic's legal profile) and doctor shares: the rules and the
//! preview of what each doctor earned in a period (frozen statements come in phase 2).

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
        return Err(CoreError::invalid("tin", ValidationRule::InvalidParams, "a TIN has digits and letters only"));
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
        return Err(CoreError::invalid("receipt_footer", ValidationRule::DocumentText, "at most 200 characters"));
    }
    let legal = clean_legal(&p.legal)?;
    set_setting(conn, actor, "fin.share_enabled", if p.share_enabled { "1" } else { "0" })?;
    set_setting(conn, actor, "fin.share_basis", match p.share_basis {
        CommissionBasis::Payment => "payment",
        CommissionBasis::Treatment => "treatment",
    })?;
    set_setting(conn, actor, "fin.receipt_footer", footer)?;
    set_setting(conn, actor, "fin.legal", &serde_json::to_string(&legal)?)?;
    let after = get_settings(conn)?;
    audit::record(conn, actor, "billing.settings", Some("setting"), None, Some(&json!(before)), Some(&json!(after)))?;
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

pub fn save_rule(conn: &Connection, actor: &Actor, p: &SaveCommissionRuleParams) -> Result<CommissionRuleInfo> {
    let doctor = check_doctor(conn, &p.doctor_id)?;
    let service = match p.service_id.as_deref().filter(|s| !s.is_empty()) {
        None => None,
        Some(s) => {
            let found: Option<String> =
                conn.query_row("SELECT id FROM service WHERE id = ?1 AND deleted_at IS NULL", [s], |r| r.get(0)).optional()?;
            Some(found.ok_or_else(|| CoreError::invalid("service_id", ValidationRule::ServiceNotFound, "unknown service"))?)
        }
    };
    let ok = match p.kind {
        CommissionKind::Percent => (0..=10_000).contains(&p.value),
        CommissionKind::Fixed => (0..=MAX_AMOUNT).contains(&p.value),
    };
    if !ok {
        return Err(CoreError::invalid("value", ValidationRule::CommissionValue, "the share value is out of range"));
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
        CoreError::api(ErrorCode::NotFound, "commission rule").on_field("id", ValidationRule::CommissionRuleNotFound)
    })?;
    audit::record(conn, actor, "commission.rule", Some("commission_rule"), Some(&id), None, Some(&json!(after)))?;
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
fn rate<'a>(rules: &'a [CommissionRuleInfo], doctor: &str, service: Option<&str>) -> Option<&'a CommissionRuleInfo> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(doctor: Option<&str>, service: Option<&str>, kind: CommissionKind, value: i64) -> CommissionRuleInfo {
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
