//! A patient's money: the account summary, the statement, receivables by age, installments due.

use std::collections::BTreeMap;

use artaveo_shared::*;
use rusqlite::{Connection, OptionalExtension};

use crate::error::Result;

use super::invoices::{plan_of, settled, Settled};
use super::ledger::*;
use super::receipts::deposit;
use super::util::*;

/// `(patient name, number, phone)`.
fn patient_names(conn: &Connection, patient_id: &str) -> Result<(String, String, Option<String>)> {
    Ok(conn
        .query_row("SELECT full_name, patient_number, phone FROM patient WHERE id = ?1", [patient_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?
        .unwrap_or_default())
}

// ───────────────────────────── installments due ─────────────────────────────

/// Every unpaid installment of posted invoices with an active plan (of one patient, or all).
pub(crate) fn open_installments(conn: &Connection, patient_id: Option<&str>) -> Result<Vec<InstallmentDue>> {
    let mut stmt = conn.prepare(
        "SELECT pp.ref_id, i.year, i.serial, i.party_id FROM fin_doc pp JOIN fin_doc i ON i.id = pp.ref_id
         WHERE pp.kind = 'pp' AND pp.status = 'posted' AND i.status = 'posted' AND (?1 IS NULL OR i.party_id = ?1)
         ORDER BY pp.id",
    )?;
    let plans: Vec<(i64, i64, i64, String)> = stmt
        .query_map([patient_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, Option<String>>(3)?.unwrap_or_default()))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let today = today();
    let mut out = Vec::new();
    for (invoice_id, year, serial, patient) in plans {
        let s = settled(conn, invoice_id)?;
        let Some(plan) = plan_of(conn, invoice_id, &s)? else { continue };
        let (name, number, phone) = patient_names(conn, &patient)?;
        let count = plan.installments.len() as i64;
        for i in plan.installments {
            if i.state == InstallmentState::Paid {
                continue;
            }
            let due = ymd(&i.due_date);
            out.push(InstallmentDue {
                patient_id: patient.clone(),
                patient_name: name.clone(),
                patient_number: number.clone(),
                phone: phone.clone(),
                invoice_id,
                invoice_number: format_number("INV", year, serial),
                no: i.no,
                count,
                due_date: i.due_date,
                amount: i.amount,
                paid: i.paid,
                state: i.state,
                days_late: days_between(due, today).max(0),
            });
        }
    }
    out.sort_by(|a, b| a.due_date.cmp(&b.due_date).then(a.patient_name.cmp(&b.patient_name)));
    Ok(out)
}

pub fn due(conn: &Connection, p: &InstallmentsDueParams) -> Result<Vec<InstallmentDue>> {
    let until = match opt_date("until", &p.until)? {
        Some(d) => d,
        None => crate::clock::today_iso(),
    };
    Ok(open_installments(conn, p.patient_id.as_deref().filter(|s| !s.is_empty()))?
        .into_iter()
        .filter(|i| i.due_date <= until)
        .filter(|i| !p.overdue_only || i.state == InstallmentState::Overdue)
        .collect())
}

// ───────────────────────────── account ─────────────────────────────

pub fn account(conn: &Connection, patient_id: &str) -> Result<PatientAccount> {
    let mut stmt =
        conn.prepare("SELECT id FROM fin_doc WHERE kind = 'inv' AND party_id = ?1 AND status = 'posted'")?;
    let ids: Vec<i64> = stmt.query_map([patient_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let mut total = Settled::default();
    let mut owed = 0;
    for id in ids {
        let s = settled(conn, id)?;
        owed += s.remaining();
        total.net += s.net;
        total.reductions += s.reductions;
        total.written_off += s.written_off;
        total.allocated += s.allocated;
    }
    let (billable_count, billable_total) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(unit_price * units), 0) FROM treatment_item
         WHERE patient_id = ?1 AND status = 'completed' AND billed_at IS NULL AND deleted_at IS NULL",
        [patient_id],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )?;
    let open = open_installments(conn, Some(patient_id))?;
    let deposit = deposit(conn, patient_id, None)?;
    Ok(PatientAccount {
        patient_id: patient_id.to_string(),
        invoiced: total.net,
        reductions: total.reductions,
        written_off: total.written_off,
        allocated: total.allocated,
        owed,
        deposit,
        balance: owed - deposit,
        billable_count,
        billable_total,
        overdue: open
            .iter()
            .filter(|i| i.state == InstallmentState::Overdue)
            .map(|i| i.amount - i.paid)
            .sum(),
        next_due: open.into_iter().next(),
    })
}

// ───────────────────────────── statement ─────────────────────────────

/// `(id, kind, year, serial, date, amount, reason or note)` of a document on the statement.
type StatementDoc = (i64, String, i64, i64, i64, i64, String);

pub fn statement(conn: &Connection, p: &StatementParams) -> Result<StatementInfo> {
    crate::patient::get_patient(conn, &p.patient_id)?;
    let (name, number, phone) = patient_names(conn, &p.patient_id)?;
    let from = opt_date("from", &p.from)?;
    let to = opt_date("to", &p.to)?.unwrap_or_else(crate::clock::today_iso);
    let (lo, hi) = (from.as_deref().map(ymd).unwrap_or(0), ymd(&to));
    let mut stmt = conn.prepare(
        "SELECT d.id, d.kind, d.year, d.serial, d.doc_date, d.amount, COALESCE(d.reason, d.note, '')
         FROM fin_doc d WHERE d.party_id = ?1 AND d.status = 'posted' AND d.kind IN ('inv', 'rc', 'rf', 'cn', 'wo')
            AND d.doc_date <= ?2
         ORDER BY d.doc_date, d.id",
    )?;
    let docs: Vec<StatementDoc> = stmt
        .query_map(rusqlite::params![p.patient_id, hi], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut names = conn.prepare_cached("SELECT group_concat(name, '، ') FROM fin_line WHERE doc_id = ?1")?;
    let mut opening = 0;
    let mut balance = 0;
    let mut lines = Vec::new();
    for (id, kind, year, serial, date, amount, text) in docs {
        let (charge, credit) = match kind.as_str() {
            "inv" => (amount, 0),
            "rf" => (0, -amount),
            _ => (0, amount),
        };
        balance += charge - credit;
        if date < lo {
            opening = balance;
            continue;
        }
        let description = if kind == "inv" {
            names.query_row([id], |r| r.get::<_, Option<String>>(0))?.unwrap_or_default()
        } else {
            text
        };
        lines.push(StatementLine {
            date: iso_of(date),
            doc_id: id,
            kind: kind_of(&kind),
            number: number_of(&kind, year, serial),
            description,
            charge,
            credit,
            balance,
        });
    }
    Ok(StatementInfo {
        patient_id: p.patient_id.clone(),
        patient_name: name,
        patient_number: number,
        phone,
        from,
        to,
        opening,
        charges: lines.iter().map(|l| l.charge).sum(),
        credits: lines.iter().map(|l| l.credit).sum(),
        closing: balance,
        lines,
    })
}

// ───────────────────────────── aging ─────────────────────────────

/// Open invoices by age (0–30, 31–60, 61–90, 90+ days) as of a day, one row per patient.
pub fn aging(conn: &Connection, p: &AgingParams) -> Result<Vec<AgingRow>> {
    let as_of = ymd(&opt_date("as_of", &p.as_of)?.unwrap_or_else(crate::clock::today_iso));
    let mut stmt = conn.prepare(
        "SELECT d.party_id, d.doc_date, d.amount
            - (SELECT COALESCE(SUM(x.amount), 0) FROM fin_doc x WHERE x.ref_id = d.id AND x.kind IN ('cn', 'wo')
                AND x.status = 'posted' AND x.doc_date <= ?1)
            - (SELECT COALESCE(SUM(l.amount), 0) FROM fin_line l JOIN fin_doc a ON a.id = l.doc_id
                WHERE l.ref_id = d.id AND a.kind IN ('rc', 'al') AND a.status = 'posted' AND a.doc_date <= ?1)
         FROM fin_doc d WHERE d.kind = 'inv' AND d.status = 'posted' AND d.doc_date <= ?1",
    )?;
    let rows: Vec<(String, i64, i64)> = stmt
        .query_map([as_of], |r| {
            Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get(1)?, r.get(2)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut by: BTreeMap<String, AgingRow> = BTreeMap::new();
    for (patient, date, left) in rows {
        if left <= 0 {
            continue;
        }
        let row = by.entry(patient.clone()).or_insert_with(|| AgingRow {
            patient_id: patient.clone(),
            patient_name: String::new(),
            patient_number: String::new(),
            phone: None,
            invoice_count: 0,
            d0_30: 0,
            d31_60: 0,
            d61_90: 0,
            d90_plus: 0,
            total: 0,
            deposit: 0,
            oldest_date: None,
            last_receipt: None,
            next_due: None,
        });
        match days_between(date, as_of) {
            ..=30 => row.d0_30 += left,
            31..=60 => row.d31_60 += left,
            61..=90 => row.d61_90 += left,
            _ => row.d90_plus += left,
        }
        row.total += left;
        row.invoice_count += 1;
        let iso = iso_of(date);
        if row.oldest_date.as_ref().is_none_or(|o| iso < *o) {
            row.oldest_date = Some(iso);
        }
    }
    let search = p
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(crate::normalize::normalize_for_search);
    let open = open_installments(conn, None)?;
    let mut out = Vec::new();
    for (patient, mut row) in by {
        let (name, number, phone) = patient_names(conn, &patient)?;
        if let Some(q) = &search {
            let hay = format!(
                "{} {} {}",
                crate::normalize::normalize_for_search(&name),
                number,
                phone.clone().unwrap_or_default()
            );
            if !hay.contains(q.as_str()) {
                continue;
            }
        }
        row.patient_name = name;
        row.patient_number = number;
        row.phone = phone;
        row.deposit = -balance(conn, DEPOSITS, Some(&patient), as_of)?;
        row.last_receipt = conn
            .query_row(
                "SELECT MAX(doc_date) FROM fin_doc WHERE party_id = ?1 AND kind = 'rc' AND status = 'posted' AND doc_date <= ?2",
                rusqlite::params![patient, as_of],
                |r| r.get::<_, Option<i64>>(0),
            )?
            .map(iso_of);
        row.next_due = open.iter().find(|i| i.patient_id == patient).map(|i| i.due_date.clone());
        out.push(row);
    }
    out.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.patient_name.cmp(&b.patient_name)));
    out.truncate(p.limit.unwrap_or(1000).min(5000) as usize);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_split_at_30_60_90_days() {
        // 2026-01-31 → 2026-03-02 is 30 days (first bucket), 31 the next.
        assert_eq!(days_between(20260131, 20260302), 30);
        assert_eq!(days_between(20260130, 20260302), 31);
    }
}
