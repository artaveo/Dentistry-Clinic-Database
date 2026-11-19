//! Money out and supplier debts: payment vouchers (PV), supplier and lab bills (BL), other money in
//! (OR) and transfers between money accounts (TR).
//!
//! PV: Dr the expense (or what is paid off) · Cr money (net) · Cr withheld tax (what was kept back).
//! BL: Dr the expense · Cr the supplier's payable. OR: Dr money · Cr capital / loans / other income.

use artaveo_shared::*;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::error::{CoreError, Result};

use super::ledger::*;
use super::util::*;

/// Lines on one voucher or bill.
const MAX_LINES: usize = 30;

fn purpose_code(p: VoucherPurpose) -> &'static str {
    match p {
        VoucherPurpose::Expense => "expense",
        VoucherPurpose::Supplier => "supplier",
        VoucherPurpose::Tax => "tax",
        VoucherPurpose::Drawing => "drawing",
        VoucherPurpose::Loan => "loan",
        VoucherPurpose::Other => "other",
        VoucherPurpose::Salary => "salary",
        VoucherPurpose::Advance => "advance",
        VoucherPurpose::Share => "share",
    }
}

fn purpose_of(s: &str) -> Option<VoucherPurpose> {
    Some(match s {
        "expense" => VoucherPurpose::Expense,
        "supplier" => VoucherPurpose::Supplier,
        "tax" => VoucherPurpose::Tax,
        "drawing" => VoucherPurpose::Drawing,
        "loan" => VoucherPurpose::Loan,
        "other" => VoucherPurpose::Other,
        "salary" => VoucherPurpose::Salary,
        "advance" => VoucherPurpose::Advance,
        "share" => VoucherPurpose::Share,
        _ => return None,
    })
}

fn source_code(s: OtherReceiptSource) -> &'static str {
    match s {
        OtherReceiptSource::Capital => "capital",
        OtherReceiptSource::Loan => "loan",
        OtherReceiptSource::Sale => "sale",
    }
}

fn source_of(s: &str) -> Option<OtherReceiptSource> {
    Some(match s {
        "capital" => OtherReceiptSource::Capital,
        "loan" => OtherReceiptSource::Loan,
        "sale" => OtherReceiptSource::Sale,
        _ => return None,
    })
}

fn wh_code(k: WithholdingKind) -> &'static str {
    match k {
        WithholdingKind::Rent => "rent",
        WithholdingKind::Contractor => "contractor",
        WithholdingKind::Wage => "wage",
    }
}

fn wh_of(s: &str) -> Option<WithholdingKind> {
    match s {
        "rent" => Some(WithholdingKind::Rent),
        "contractor" => Some(WithholdingKind::Contractor),
        _ => None,
    }
}

pub(crate) fn wh_account(k: WithholdingKind) -> i64 {
    match k {
        WithholdingKind::Rent => TAX_RENT,
        WithholdingKind::Contractor => TAX_CONTRACTOR,
        WithholdingKind::Wage => TAX_WAGE,
    }
}

fn check_supplier(conn: &Connection, id: &str) -> Result<String> {
    conn.query_row("SELECT id FROM supplier WHERE id = ?1 AND deleted_at IS NULL", [id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| {
            CoreError::invalid("supplier_id", ValidationRule::SupplierNotFound, "unknown supplier")
        })
}

/// An account a bill or an "other" payment may be booked to: an expense account or equipment.
fn cost_account(conn: &Connection, id: i64) -> Result<i64> {
    if id == 1510 && account_row(conn, id)?.is_some_and(|a| a.is_active) {
        return Ok(id);
    }
    expense_account(conn, "lines", id)
}

/// A checked line: `(account, gross, withheld, withholding kind, description)`.
type Line = (i64, i64, i64, Option<WithholdingKind>, Option<String>);

fn check_lines(conn: &Connection, lines: &[NewVoucherLine], allow_withholding: bool) -> Result<Vec<Line>> {
    if lines.is_empty() || lines.len() > MAX_LINES {
        return Err(CoreError::invalid("lines", ValidationRule::Required, "1 to 30 lines"));
    }
    let mut out = Vec::with_capacity(lines.len());
    for l in lines {
        let account = cost_account(conn, l.account_id)?;
        let amount = money("amount", l.amount, false)?;
        let withheld = money("withheld", l.withheld, true)?;
        if withheld > 0 && (!allow_withholding || l.withholding.is_none()) {
            return Err(CoreError::invalid(
                "withholding",
                ValidationRule::Required,
                "say which tax was withheld",
            ));
        }
        if withheld > 0 && l.withholding == Some(WithholdingKind::Wage) {
            return Err(CoreError::invalid(
                "withholding",
                ValidationRule::WithheldRange,
                "salary tax is withheld by the payroll, not on an expense",
            ));
        }
        if withheld > amount {
            return Err(CoreError::invalid(
                "withheld",
                ValidationRule::WithheldRange,
                "more tax withheld than the line",
            ));
        }
        let desc = optional("description", &l.description, 200)?;
        out.push((account, amount, withheld, if withheld > 0 { l.withholding } else { None }, desc));
    }
    Ok(out)
}

fn write_lines(conn: &Connection, doc_id: i64, lines: &[Line], due: Option<i64>) -> Result<()> {
    for (i, (account, amount, withheld, kind, desc)) in lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO fin_line(doc_id, no, account_id, amount, withheld, wh_kind, name, due_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                doc_id,
                i as i64 + 1,
                account,
                amount,
                Some(*withheld).filter(|w| *w > 0),
                kind.map(wh_code),
                desc,
                due
            ],
        )?;
    }
    Ok(())
}

// ───────────────────────────── reading ─────────────────────────────

const VOUCHER_SELECT: &str =
    "SELECT d.id, d.kind, d.year, d.serial, d.doc_date, d.created_at, d.account_id, d.purpose,
    d.party_id, s.name, d.name, d.reference, d.note, d.status, d.void_reason, d.print_count, d.created_by,
    d.party, COALESCE(s.name, e.full_name, dr.full_name)
    FROM fin_doc d LEFT JOIN supplier s ON s.id = d.party_id AND d.party = 'supplier'
    LEFT JOIN fin_employee e ON e.id = d.party_id AND d.party = 'employee'
    LEFT JOIN doctor dr ON dr.id = d.party_id AND d.party = 'doctor'";

struct Row {
    id: i64,
    kind: String,
    year: i64,
    serial: i64,
    date: i64,
    created_at: i64,
    account_id: Option<i64>,
    purpose: Option<String>,
    party_id: Option<String>,
    supplier_name: Option<String>,
    name: Option<String>,
    reference: Option<String>,
    note: Option<String>,
    status: String,
    void_reason: Option<String>,
    print_count: i64,
    created_by: Option<String>,
    party: Option<String>,
    party_name: Option<String>,
}

fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        kind: r.get(1)?,
        year: r.get(2)?,
        serial: r.get(3)?,
        date: r.get(4)?,
        created_at: r.get(5)?,
        account_id: r.get(6)?,
        purpose: r.get(7)?,
        party_id: r.get(8)?,
        supplier_name: r.get(9)?,
        name: r.get(10)?,
        reference: r.get(11)?,
        note: r.get(12)?,
        status: r.get(13)?,
        void_reason: r.get(14)?,
        print_count: r.get(15)?,
        created_by: r.get(16)?,
        party: r.get(17)?,
        party_name: r.get(18)?,
    })
}

fn finish(conn: &Connection, x: Row) -> Result<VoucherInfo> {
    let mut stmt = conn.prepare_cached(
        "SELECT no, account_id, name, amount, COALESCE(withheld, 0), wh_kind, due_date FROM fin_line WHERE doc_id = ?1 ORDER BY no",
    )?;
    let mut due: Option<i64> = None;
    let lines: Vec<VoucherLineInfo> = stmt
        .query_map([x.id], |r| {
            Ok((
                VoucherLineInfo {
                    no: r.get(0)?,
                    account_id: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    description: r.get(2)?,
                    amount: r.get(3)?,
                    withheld: r.get(4)?,
                    withholding: r.get::<_, Option<String>>(5)?.as_deref().and_then(wh_of),
                },
                r.get::<_, Option<i64>>(6)?,
            ))
        })?
        .map(|row| {
            row.map(|(line, d)| {
                due = due.or(d);
                line
            })
        })
        .collect::<rusqlite::Result<_>>()?;
    let gross: i64 = lines.iter().map(|l| l.amount).sum();
    let withheld: i64 = lines.iter().map(|l| l.withheld).sum();
    let kind = kind_of(&x.kind);
    Ok(VoucherInfo {
        created_by_name: user_name(conn, &x.created_by)?,
        id: x.id,
        kind,
        number: number_of(&x.kind, x.year, x.serial),
        date: iso_of(x.date),
        created_at: iso_at(x.created_at),
        account_id: x.account_id,
        purpose: if kind == DocKind::Pv { x.purpose.as_deref().and_then(purpose_of) } else { None },
        source: if kind == DocKind::Or { x.purpose.as_deref().and_then(source_of) } else { None },
        supplier_id: x.party_id.clone().filter(|_| x.party.as_deref() == Some("supplier")),
        supplier_name: x.supplier_name,
        employee_id: x.party_id.clone().filter(|_| x.party.as_deref() == Some("employee")),
        doctor_id: x.party_id.clone().filter(|_| x.party.as_deref() == Some("doctor")),
        party_name: x.party_name,
        name: x.name,
        reference: x.reference,
        lines,
        gross,
        withheld,
        net: gross - withheld,
        due_date: due.map(iso_of),
        note: x.note,
        status: status_of(&x.status),
        void_reason: x.void_reason,
        print_count: x.print_count,
    })
}

pub fn get(conn: &Connection, id: i64) -> Result<VoucherInfo> {
    let row = conn
        .query_row(
            &format!("{VOUCHER_SELECT} WHERE d.id = ?1 AND d.kind IN ('pv', 'bl', 'or')"),
            [id],
            map_row,
        )
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("voucher {id}"))
                .on_field("id", ValidationRule::DocNotFound)
        })?;
    finish(conn, row)
}

pub fn list(conn: &Connection, p: &VoucherListParams) -> Result<Vec<VoucherInfo>> {
    let mut filter = String::from(" WHERE d.kind IN ('pv', 'bl', 'or')");
    let mut args: Vec<Value> = Vec::new();
    if let Some(k) = p.kind {
        filter.push_str(" AND d.kind = ?");
        args.push(Value::Text(kind_code(k).to_string()));
    }
    if let Some(purpose) = p.purpose {
        filter.push_str(" AND d.kind = 'pv' AND d.purpose = ?");
        args.push(Value::Text(purpose_code(purpose).to_string()));
    }
    if let Some(a) = p.account_id {
        filter.push_str(" AND (d.account_id = ? OR EXISTS(SELECT 1 FROM fin_line l WHERE l.doc_id = d.id AND l.account_id = ?))");
        args.push(Value::Integer(a));
        args.push(Value::Integer(a));
    }
    if let Some(s) = p.supplier_id.as_deref().filter(|s| !s.is_empty()) {
        filter.push_str(" AND d.party_id = ?");
        args.push(Value::Text(s.to_string()));
    }
    if let Some(from) = opt_date("from", &p.from)? {
        filter.push_str(" AND d.doc_date >= ?");
        args.push(Value::Integer(ymd(&from)));
    }
    if let Some(to) = opt_date("to", &p.to)? {
        filter.push_str(" AND d.doc_date <= ?");
        args.push(Value::Integer(ymd(&to)));
    }
    args.push(Value::Integer(p.limit.unwrap_or(200).min(MAX_LIST) as i64));
    args.push(Value::Integer(p.offset.unwrap_or(0) as i64));
    let mut stmt = conn
        .prepare(&format!("{VOUCHER_SELECT}{filter} ORDER BY d.doc_date DESC, d.id DESC LIMIT ? OFFSET ?"))?;
    let rows: Vec<Row> =
        stmt.query_map(params_from_iter(args.iter()), map_row)?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|r| finish(conn, r)).collect()
}

// ───────────────────────────── payment vouchers ─────────────────────────────

pub fn create_voucher(conn: &Connection, actor: &Actor, p: &CreateVoucherParams) -> Result<VoucherInfo> {
    let date = doc_day("date", &p.date)?;
    let account = money_account(conn, "account_id", p.account_id)?;
    let supplier = match p.supplier_id.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => Some(check_supplier(conn, s)?),
        None => None,
    };
    let name = optional("name", &p.name, 100)?;
    let reference = optional("reference", &p.reference, 60)?;
    let note = optional("note", &p.note, 300)?;
    let mut employee: Option<String> = None;
    let mut doctor: Option<String> = None;
    // (lines to write, ledger entries)
    let (lines, mut entries): (Vec<Line>, Vec<Entry>) = match p.purpose {
        VoucherPurpose::Expense | VoucherPurpose::Other => {
            let lines = check_lines(conn, &p.lines, true)?;
            let entries = lines.iter().map(|l| dr(l.0, l.1)).collect();
            (lines, entries)
        }
        VoucherPurpose::Supplier => {
            let s = supplier.as_deref().ok_or_else(|| {
                CoreError::invalid("supplier_id", ValidationRule::Required, "choose the supplier being paid")
            })?;
            let amount = money("amount", p.amount, false)?;
            (vec![(PAYABLE, amount, 0, None, None)], vec![dr(PAYABLE, amount).of(s)])
        }
        VoucherPurpose::Tax => {
            let kind = p.withholding.ok_or_else(|| {
                CoreError::invalid("withholding", ValidationRule::Required, "say which withheld tax is paid")
            })?;
            let amount = money("amount", p.amount, false)?;
            let acc = wh_account(kind);
            (vec![(acc, amount, 0, None, None)], vec![dr(acc, amount)])
        }
        VoucherPurpose::Salary | VoucherPurpose::Advance => {
            let e = p.employee_id.as_deref().filter(|s| !s.is_empty()).ok_or_else(|| {
                CoreError::invalid("employee_id", ValidationRule::Required, "choose the employee being paid")
            })?;
            let e = super::staff::get_employee(conn, e)?;
            let amount = money("amount", p.amount, false)?;
            let acc = if p.purpose == VoucherPurpose::Salary { WAGES_PAYABLE } else { ADVANCES };
            if p.purpose == VoucherPurpose::Salary && amount > e.owed {
                return Err(CoreError::invalid(
                    "amount",
                    ValidationRule::PaymentExceeds,
                    format!("the clinic owes {} only {}", e.full_name, e.owed),
                ));
            }
            employee = Some(e.id.clone());
            (vec![(acc, amount, 0, None, None)], vec![dr(acc, amount).of(&e.id)])
        }
        VoucherPurpose::Share => {
            let d = check_doctor(conn, &p.doctor_id)?.ok_or_else(|| {
                CoreError::invalid("doctor_id", ValidationRule::Required, "choose the doctor being paid")
            })?;
            let amount = money("amount", p.amount, false)?;
            let owed = -balance(conn, SHARES_PAYABLE, Some(&d), 99_991_231)?;
            if amount > owed {
                return Err(CoreError::invalid(
                    "amount",
                    ValidationRule::PaymentExceeds,
                    format!("approved shares not yet paid are {owed}"),
                ));
            }
            doctor = Some(d.clone());
            (vec![(SHARES_PAYABLE, amount, 0, None, None)], vec![dr(SHARES_PAYABLE, amount).of(&d)])
        }
        VoucherPurpose::Drawing | VoucherPurpose::Loan => {
            let acc = if p.purpose == VoucherPurpose::Drawing { DRAWINGS } else { LOANS };
            let amount = money("amount", p.amount, false)?;
            (vec![(acc, amount, 0, None, None)], vec![dr(acc, amount)])
        }
    };
    let gross: i64 = lines.iter().map(|l| l.1).sum();
    let withheld: i64 = lines.iter().map(|l| l.2).sum();
    if gross > MAX_AMOUNT {
        return Err(CoreError::invalid("amount", ValidationRule::BillingAmount, "the total is too large"));
    }
    for l in &lines {
        if let Some(k) = l.3 {
            entries.push(cr(wh_account(k), l.2));
        }
    }
    entries.push(cr(account, gross - withheld));
    let (id, number) = insert_doc(
        conn,
        actor,
        &NewDoc {
            kind: Some(DocKind::Pv),
            date,
            supplier_id: supplier.as_deref(),
            employee_id: employee.as_deref(),
            doctor_id: doctor.as_deref(),
            account_id: Some(account),
            amount: gross - withheld,
            purpose: Some(purpose_code(p.purpose)),
            name: name.as_deref(),
            reference: reference.as_deref(),
            note: note.as_deref(),
            ..Default::default()
        },
    )?;
    write_lines(conn, id, &lines, None)?;
    post(conn, id, date, entries)?;
    audit::record(
        conn,
        actor,
        "voucher.create",
        Some("fin_doc"),
        Some(&id.to_string()),
        None,
        Some(
            &json!({"number": number, "purpose": purpose_code(p.purpose), "gross": gross, "withheld": withheld}),
        ),
    )?;
    get(conn, id)
}

// ───────────────────────────── bills ─────────────────────────────

pub fn create_bill(conn: &Connection, actor: &Actor, p: &CreateBillParams) -> Result<VoucherInfo> {
    let supplier = check_supplier(conn, &p.supplier_id)?;
    let date = doc_day("date", &p.date)?;
    let due = opt_date("due_date", &p.due_date)?.map(|d| ymd(&d));
    let reference = optional("reference", &p.reference, 60)?;
    let note = optional("note", &p.note, 300)?;
    let lines = check_lines(conn, &p.lines, false)?;
    let total: i64 = lines.iter().map(|l| l.1).sum();
    let (id, number) = insert_doc(
        conn,
        actor,
        &NewDoc {
            kind: Some(DocKind::Bl),
            date,
            supplier_id: Some(&supplier),
            amount: total,
            purpose: Some("manual"),
            reference: reference.as_deref(),
            note: note.as_deref(),
            ..Default::default()
        },
    )?;
    write_lines(conn, id, &lines, due)?;
    let mut entries: Vec<Entry> = lines.iter().map(|l| dr(l.0, l.1)).collect();
    entries.push(cr(PAYABLE, total).of(&supplier));
    post(conn, id, date, entries)?;
    audit::record(
        conn,
        actor,
        "bill.create",
        Some("supplier"),
        Some(&supplier),
        None,
        Some(&json!({"id": id, "number": number, "total": total})),
    )?;
    get(conn, id)
}

/// The bill an inventory document (purchase, lab case, return) stands for.
pub(crate) struct ExtBill<'a> {
    /// The purchase, lab case or stock movement id.
    pub ext_id: &'a str,
    /// `purchase`, `lab` or `return`.
    pub source: &'a str,
    pub supplier_id: &'a str,
    /// What the clinic owes for it (negative for a return the supplier credits); 0 = no bill.
    pub amount: i64,
    /// `yyyymmdd`.
    pub date: i64,
    pub memo: &'a str,
}

/// Keeps the bill made from an inventory document equal to it: the old bill is voided and a new one
/// written when the amount changes.
pub(crate) fn sync_bill(conn: &Connection, actor: &Actor, b: &ExtBill<'_>) -> Result<()> {
    let ExtBill { ext_id, source, supplier_id, amount, date, memo } = *b;
    let current: Option<(i64, i64, Option<String>)> = conn
        .query_row(
            "SELECT id, amount, party_id FROM fin_doc WHERE ext_id = ?1 AND kind = 'bl' AND status = 'posted'",
            [ext_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let same = current.as_ref().is_some_and(|c| c.1 == amount && c.2.as_deref() == Some(supplier_id));
    if same || (current.is_none() && amount == 0) {
        return Ok(());
    }
    if let Some((id, _, _)) = current {
        void_doc(conn, actor, &head(conn, id)?, memo)?;
    }
    if amount == 0 {
        return Ok(());
    }
    let account = if source == "lab" { LAB } else { MATERIALS };
    let (id, _) = insert_doc(
        conn,
        actor,
        &NewDoc {
            kind: Some(DocKind::Bl),
            date,
            supplier_id: Some(supplier_id),
            ext_id: Some(ext_id),
            amount,
            purpose: Some(source),
            note: Some(memo),
            ..Default::default()
        },
    )?;
    conn.execute(
        "INSERT INTO fin_line(doc_id, no, account_id, amount, name) VALUES (?1, 1, ?2, ?3, ?4)",
        params![id, account, amount, memo],
    )?;
    post(conn, id, date, vec![dr(account, amount), cr(PAYABLE, amount).of(supplier_id)])
}

// ───────────────────────────── other money in, transfers ─────────────────────────────

pub fn create_other_receipt(
    conn: &Connection,
    actor: &Actor,
    p: &CreateOtherReceiptParams,
) -> Result<VoucherInfo> {
    let date = doc_day("date", &p.date)?;
    let account = money_account(conn, "account_id", p.account_id)?;
    let amount = money("amount", p.amount, false)?;
    let name = optional("name", &p.name, 100)?;
    let note = optional("note", &p.note, 300)?;
    let credit = match p.source {
        OtherReceiptSource::Capital => CAPITAL,
        OtherReceiptSource::Loan => LOANS,
        OtherReceiptSource::Sale => OTHER_INCOME,
    };
    let (id, number) = insert_doc(
        conn,
        actor,
        &NewDoc {
            kind: Some(DocKind::Or),
            date,
            account_id: Some(account),
            amount,
            purpose: Some(source_code(p.source)),
            name: name.as_deref(),
            note: note.as_deref(),
            ..Default::default()
        },
    )?;
    conn.execute(
        "INSERT INTO fin_line(doc_id, no, account_id, amount) VALUES (?1, 1, ?2, ?3)",
        params![id, credit, amount],
    )?;
    post(conn, id, date, vec![dr(account, amount), cr(credit, amount)])?;
    audit::record(
        conn,
        actor,
        "other_receipt.create",
        Some("fin_doc"),
        Some(&id.to_string()),
        None,
        Some(&json!({"number": number, "source": source_code(p.source), "amount": amount})),
    )?;
    get(conn, id)
}

pub fn create_transfer(conn: &Connection, actor: &Actor, p: &CreateTransferParams) -> Result<DocSummary> {
    let date = doc_day("date", &p.date)?;
    let from = money_account(conn, "from_account_id", p.from_account_id)?;
    let to = money_account(conn, "to_account_id", p.to_account_id)?;
    if from == to {
        return Err(CoreError::invalid(
            "to_account_id",
            ValidationRule::SameAccount,
            "choose two different accounts",
        ));
    }
    let amount = money("amount", p.amount, false)?;
    let reference = optional("reference", &p.reference, 60)?;
    let note = optional("note", &p.note, 300)?;
    let (id, number) = insert_doc(
        conn,
        actor,
        &NewDoc {
            kind: Some(DocKind::Tr),
            date,
            account_id: Some(from),
            account2_id: Some(to),
            amount,
            reference: reference.as_deref(),
            note: note.as_deref(),
            ..Default::default()
        },
    )?;
    post(conn, id, date, vec![dr(to, amount), cr(from, amount)])?;
    audit::record(
        conn,
        actor,
        "transfer.create",
        Some("fin_doc"),
        Some(&id.to_string()),
        None,
        Some(&json!({"number": number, "from": from, "to": to, "amount": amount})),
    )?;
    Ok(summary(&head(conn, id)?))
}
