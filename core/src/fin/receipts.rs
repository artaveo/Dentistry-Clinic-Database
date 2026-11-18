//! Money from patients: receipts (RC), allocation of a deposit to invoices or back (AL), refunds (RF).
//!
//! A receipt posts Dr money · Cr receivable (what it pays on invoices) · Cr deposits (the rest).
//! An allocation moves deposit to invoices (Dr deposits, Cr receivable) or back (the opposite).
//! A refund pays a deposit back (Dr deposits, Cr money).

use artaveo_shared::*;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::error::{CoreError, Result};

use super::invoices::settled;
use super::ledger::*;
use super::util::*;

/// The patient's deposit (money held, not yet on an invoice). With `as_of_doc`, as it stood right
/// after that document (voids made later are left out).
pub(crate) fn deposit(conn: &Connection, patient_id: &str, as_of_doc: Option<i64>) -> Result<i64> {
    Ok(-party_balance(conn, DEPOSITS, patient_id, as_of_doc)?)
}

/// What the patient owes on invoices (receivable), the same way.
pub(crate) fn owed(conn: &Connection, patient_id: &str, as_of_doc: Option<i64>) -> Result<i64> {
    party_balance(conn, RECEIVABLE, patient_id, as_of_doc)
}

fn party_balance(conn: &Connection, account: i64, party: &str, as_of_doc: Option<i64>) -> Result<i64> {
    match as_of_doc {
        None => scalar(
            conn,
            "SELECT SUM(amount) FROM fin_gl WHERE party_id = ?1 AND account_id = ?2",
            &[&party, &account],
        ),
        Some(doc) => {
            let at: i64 =
                conn.query_row("SELECT created_at FROM fin_doc WHERE id = ?1", [doc], |r| r.get(0))?;
            scalar(
                conn,
                "SELECT SUM(g.amount) FROM fin_gl g JOIN fin_doc d ON d.id = g.doc_id
                 WHERE g.party_id = ?1 AND g.account_id = ?2 AND g.doc_id <= ?3 AND (g.rev = 0 OR d.voided_at <= ?4)",
                &[&party, &account, &doc, &at],
            )
        }
    }
}

/// After a void, a patient's books must still make sense: no negative deposit, no invoice paid
/// beyond its net. Otherwise later documents depend on the voided one.
pub(crate) fn check_books(conn: &Connection, patient_id: &str) -> Result<()> {
    if deposit(conn, patient_id, None)? < 0 {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::DocNotVoidable,
            "the deposit from this document was already used; take it back first",
        ));
    }
    let over: i64 = scalar(
        conn,
        "SELECT COUNT(*) FROM fin_doc d WHERE d.kind = 'inv' AND d.party_id = ?1 AND d.status = 'posted'
         AND d.amount < (SELECT COALESCE(SUM(x.amount), 0) FROM fin_doc x WHERE x.ref_id = d.id AND x.kind IN ('cn', 'wo')
                AND x.status = 'posted')
            + (SELECT COALESCE(SUM(l.amount), 0) FROM fin_line l JOIN fin_doc a ON a.id = l.doc_id
                WHERE l.ref_id = d.id AND a.kind IN ('rc', 'al') AND a.status = 'posted')",
        &[&patient_id],
    )?;
    if over > 0 {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::DocNotVoidable,
            "an invoice would then be settled beyond its amount",
        ));
    }
    Ok(())
}

// ───────────────────────────── reading ─────────────────────────────

const RECEIPT_SELECT: &str = "SELECT d.id, d.kind, d.year, d.serial, d.party_id, p.full_name, p.patient_number, d.doc_date,
    d.created_at, d.account_id, d.amount, d.name, d.note, d.reason, d.status, d.void_reason, d.print_count, d.created_by
    FROM fin_doc d JOIN patient p ON p.id = d.party_id";

struct Row {
    id: i64,
    kind: DocKind,
    number: String,
    patient_id: String,
    patient_name: String,
    patient_number: String,
    date: i64,
    created_at: i64,
    account_id: Option<i64>,
    amount: i64,
    name: Option<String>,
    note: Option<String>,
    reason: Option<String>,
    status: DocStatus,
    void_reason: Option<String>,
    print_count: i64,
    created_by: Option<String>,
}

fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    let kind: String = r.get(1)?;
    Ok(Row {
        id: r.get(0)?,
        number: number_of(&kind, r.get(2)?, r.get(3)?),
        kind: kind_of(&kind),
        patient_id: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        patient_name: r.get(5)?,
        patient_number: r.get(6)?,
        date: r.get(7)?,
        created_at: r.get(8)?,
        account_id: r.get(9)?,
        amount: r.get(10)?,
        name: r.get(11)?,
        note: r.get(12)?,
        reason: r.get(13)?,
        status: status_of(&r.get::<_, String>(14)?),
        void_reason: r.get(15)?,
        print_count: r.get(16)?,
        created_by: r.get(17)?,
    })
}

fn finish(conn: &Connection, x: Row) -> Result<ReceiptInfo> {
    let mut stmt = conn.prepare_cached(
        "SELECT l.ref_id, i.year, i.serial, l.amount FROM fin_line l JOIN fin_doc i ON i.id = l.ref_id
         WHERE l.doc_id = ?1 ORDER BY l.no",
    )?;
    let allocations: Vec<AllocationLine> = stmt
        .query_map([x.id], |r| {
            Ok(AllocationLine {
                invoice_id: r.get(0)?,
                invoice_number: format_number("INV", r.get(1)?, r.get(2)?),
                amount: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let allocated: i64 = allocations.iter().map(|a| a.amount).sum();
    let to_deposit = match x.kind {
        DocKind::Rc => x.amount - allocated,
        DocKind::Al => -allocated,
        _ => -x.amount,
    };
    Ok(ReceiptInfo {
        owed_after: owed(conn, &x.patient_id, Some(x.id))?,
        deposit_after: deposit(conn, &x.patient_id, Some(x.id))?,
        created_by_name: user_name(conn, &x.created_by)?,
        id: x.id,
        kind: x.kind,
        number: x.number,
        patient_id: x.patient_id,
        patient_name: x.patient_name,
        patient_number: x.patient_number,
        date: iso_of(x.date),
        created_at: iso_at(x.created_at),
        account_id: x.account_id,
        amount: x.amount,
        allocations,
        to_deposit,
        payer_name: x.name,
        note: x.note,
        reason: x.reason,
        status: x.status,
        void_reason: x.void_reason,
        print_count: x.print_count,
    })
}

pub fn get(conn: &Connection, id: i64) -> Result<ReceiptInfo> {
    let row = conn
        .query_row(
            &format!("{RECEIPT_SELECT} WHERE d.id = ?1 AND d.kind IN ('rc', 'al', 'rf')"),
            [id],
            map_row,
        )
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("receipt {id}"))
                .on_field("id", ValidationRule::DocNotFound)
        })?;
    finish(conn, row)
}

pub fn list(conn: &Connection, p: &ReceiptListParams) -> Result<Vec<ReceiptInfo>> {
    let mut filter = String::from(" WHERE d.kind IN ('rc', 'al', 'rf')");
    let mut args: Vec<Value> = Vec::new();
    if let Some(k) = p.kind {
        filter.push_str(" AND d.kind = ?");
        args.push(Value::Text(kind_code(k).to_string()));
    }
    if let Some(pid) = p.patient_id.as_deref().filter(|s| !s.is_empty()) {
        filter.push_str(" AND d.party_id = ?");
        args.push(Value::Text(pid.to_string()));
    }
    if let Some(a) = p.account_id {
        filter.push_str(" AND d.account_id = ?");
        args.push(Value::Integer(a));
    }
    if let Some(from) = opt_date("from", &p.from)? {
        filter.push_str(" AND d.doc_date >= ?");
        args.push(Value::Integer(ymd(&from)));
    }
    if let Some(to) = opt_date("to", &p.to)? {
        filter.push_str(" AND d.doc_date <= ?");
        args.push(Value::Integer(ymd(&to)));
    }
    if let Some(q) = p.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let like = format!("%{}%", crate::normalize::normalize_for_search(q));
        filter.push_str(&format!(
            " AND ({NUMBER_SQL} LIKE ? OR p.patient_number LIKE ? OR p.full_name LIKE ?)"
        ));
        for _ in 0..3 {
            args.push(Value::Text(like.clone()));
        }
    }
    let limit = p.limit.unwrap_or(100).min(MAX_LIST) as i64;
    let offset = p.offset.unwrap_or(0) as i64;
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&format!("{RECEIPT_SELECT}{filter} ORDER BY d.id DESC LIMIT ? OFFSET ?"))?;
    let rows: Vec<Row> =
        stmt.query_map(params_from_iter(args.iter()), map_row)?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|r| finish(conn, r)).collect()
}

// ───────────────────────────── checks ─────────────────────────────

/// An open invoice of this patient and what it still owes.
fn open_invoice(conn: &Connection, patient_id: &str, invoice_id: i64) -> Result<(String, i64)> {
    let row: Option<(Option<String>, String, i64, i64)> = conn
        .query_row(
            "SELECT party_id, status, year, serial FROM fin_doc WHERE id = ?1 AND kind = 'inv'",
            [invoice_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    match row {
        Some((Some(p), status, year, serial)) if p == patient_id && status == "posted" => {
            Ok((format_number("INV", year, serial), settled(conn, invoice_id)?.remaining()))
        }
        _ => Err(CoreError::invalid(
            "allocations",
            ValidationRule::AllocationInvalid,
            "that invoice is not an open invoice of this patient",
        )),
    }
}

/// Checks the allocations of a receipt or deposit move; returns them as `(invoice, amount)`.
fn checked(
    conn: &Connection,
    patient_id: &str,
    list: &[NewAllocation],
    allow_negative: bool,
) -> Result<Vec<(i64, i64)>> {
    let mut out: Vec<(i64, i64)> = Vec::new();
    for a in list {
        if a.amount == 0 {
            continue;
        }
        if out.iter().any(|x| x.0 == a.invoice_id) || (a.amount < 0 && !allow_negative) {
            return Err(CoreError::invalid(
                "allocations",
                ValidationRule::AllocationInvalid,
                "one line per invoice, and only positive amounts here",
            ));
        }
        if a.amount.abs() > MAX_AMOUNT {
            return Err(CoreError::invalid(
                "allocations",
                ValidationRule::BillingAmount,
                "amount out of range",
            ));
        }
        let (_, remaining) = open_invoice(conn, patient_id, a.invoice_id)?;
        if a.amount > remaining {
            return Err(CoreError::invalid(
                "allocations",
                ValidationRule::PaymentExceeds,
                "more than what the invoice still owes",
            ));
        }
        if a.amount < 0 && -a.amount > settled(conn, a.invoice_id)?.allocated {
            return Err(CoreError::invalid(
                "allocations",
                ValidationRule::AllocationInvalid,
                "more than was put towards that invoice",
            ));
        }
        out.push((a.invoice_id, a.amount));
    }
    Ok(out)
}

/// The patient's open invoices, oldest first, with what each still owes.
fn open_invoices(conn: &Connection, patient_id: &str) -> Result<Vec<(i64, i64)>> {
    let mut stmt = conn
        .prepare("SELECT id FROM fin_doc WHERE kind = 'inv' AND party_id = ?1 AND status = 'posted' ORDER BY doc_date, id")?;
    let ids: Vec<i64> = stmt.query_map([patient_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for id in ids {
        let left = settled(conn, id)?.remaining();
        if left > 0 {
            out.push((id, left));
        }
    }
    Ok(out)
}

fn write_lines(conn: &Connection, doc_id: i64, lines: &[(i64, i64)]) -> Result<()> {
    for (i, (invoice, amount)) in lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO fin_line(doc_id, no, amount, ref_id) VALUES (?1, ?2, ?3, ?4)",
            params![doc_id, i as i64 + 1, amount, invoice],
        )?;
    }
    Ok(())
}

// ───────────────────────────── writing ─────────────────────────────

pub fn create(conn: &Connection, actor: &Actor, p: &CreateReceiptParams) -> Result<ReceiptInfo> {
    check_patient(conn, &p.patient_id)?;
    let account = money_account(conn, "account_id", p.account_id)?;
    let amount = money("amount", p.amount, false)?;
    let lines = if p.allocations.is_empty() && p.auto_allocate {
        let mut left = amount;
        let mut out = Vec::new();
        for (id, owes) in open_invoices(conn, &p.patient_id)? {
            if left == 0 {
                break;
            }
            let take = owes.min(left);
            out.push((id, take));
            left -= take;
        }
        out
    } else {
        checked(conn, &p.patient_id, &p.allocations, false)?
    };
    let allocated: i64 = lines.iter().map(|l| l.1).sum();
    if allocated > amount {
        return Err(CoreError::invalid(
            "allocations",
            ValidationRule::AllocationInvalid,
            "the allocations are more than the money received",
        ));
    }
    let payer = optional("payer_name", &p.payer_name, 100)?;
    let note = optional("note", &p.note, 300)?;
    let date = today();
    let (id, number) = insert_doc(
        conn,
        actor,
        &NewDoc {
            kind: Some(DocKind::Rc),
            date,
            patient_id: Some(&p.patient_id),
            account_id: Some(account),
            amount,
            name: payer.as_deref(),
            note: note.as_deref(),
            ..Default::default()
        },
    )?;
    write_lines(conn, id, &lines)?;
    post(
        conn,
        id,
        date,
        vec![
            dr(account, amount),
            cr(RECEIVABLE, allocated).of(&p.patient_id),
            cr(DEPOSITS, amount - allocated).of(&p.patient_id),
        ],
    )?;
    audit::record(
        conn,
        actor,
        "receipt.create",
        Some("patient"),
        Some(&p.patient_id),
        None,
        Some(
            &json!({"id": id, "number": number, "amount": amount, "allocated": allocated, "account": account}),
        ),
    )?;
    get(conn, id)
}

pub fn allocate(conn: &Connection, actor: &Actor, p: &AllocateParams) -> Result<ReceiptInfo> {
    check_patient(conn, &p.patient_id)?;
    let lines = checked(conn, &p.patient_id, &p.allocations, true)?;
    if lines.is_empty() {
        return Err(CoreError::invalid("allocations", ValidationRule::Required, "nothing to allocate"));
    }
    let back = lines.iter().any(|l| l.1 < 0);
    let reason = optional("reason", &p.reason, 300)?;
    if back && reason.is_none() {
        return Err(CoreError::invalid(
            "reason",
            ValidationRule::Required,
            "say why money is taken off an invoice",
        ));
    }
    let net: i64 = lines.iter().map(|l| l.1).sum();
    if net > deposit(conn, &p.patient_id, None)? {
        return Err(CoreError::invalid(
            "allocations",
            ValidationRule::InsufficientCredit,
            "the deposit is smaller than that",
        ));
    }
    let date = today();
    let (id, number) = insert_doc(
        conn,
        actor,
        &NewDoc {
            kind: Some(DocKind::Al),
            date,
            patient_id: Some(&p.patient_id),
            amount: net,
            reason: reason.as_deref(),
            ..Default::default()
        },
    )?;
    write_lines(conn, id, &lines)?;
    post(conn, id, date, vec![dr(DEPOSITS, net).of(&p.patient_id), cr(RECEIVABLE, net).of(&p.patient_id)])?;
    audit::record(
        conn,
        actor,
        if back { "allocation.back" } else { "allocation.create" },
        Some("patient"),
        Some(&p.patient_id),
        None,
        Some(&json!({"id": id, "number": number, "lines": lines, "reason": reason})),
    )?;
    get(conn, id)
}

pub fn refund(conn: &Connection, actor: &Actor, p: &RefundParams) -> Result<ReceiptInfo> {
    check_patient(conn, &p.patient_id)?;
    let account = money_account(conn, "account_id", p.account_id)?;
    let amount = money("amount", p.amount, false)?;
    let reason = required("reason", &p.reason, 300)?;
    if let Some(invoice) = p.from_invoice_id {
        if settled(conn, invoice)?.allocated < amount {
            return Err(CoreError::invalid(
                "amount",
                ValidationRule::RefundExceeds,
                "more than was paid on that invoice",
            ));
        }
        allocate(
            conn,
            actor,
            &AllocateParams {
                patient_id: p.patient_id.clone(),
                allocations: vec![NewAllocation { invoice_id: invoice, amount: -amount }],
                reason: Some(reason.clone()),
            },
        )?;
    }
    if amount > deposit(conn, &p.patient_id, None)? {
        return Err(CoreError::invalid(
            "amount",
            ValidationRule::InsufficientCredit,
            "the deposit is smaller than that",
        ));
    }
    let date = today();
    let (id, number) = insert_doc(
        conn,
        actor,
        &NewDoc {
            kind: Some(DocKind::Rf),
            date,
            patient_id: Some(&p.patient_id),
            account_id: Some(account),
            amount,
            reason: Some(&reason),
            ..Default::default()
        },
    )?;
    post(conn, id, date, vec![dr(DEPOSITS, amount).of(&p.patient_id), cr(account, amount)])?;
    audit::record(
        conn,
        actor,
        "payment.refund",
        Some("patient"),
        Some(&p.patient_id),
        None,
        Some(
            &json!({"id": id, "number": number, "amount": amount, "reason": reason, "invoice": p.from_invoice_id}),
        ),
    )?;
    get(conn, id)
}
