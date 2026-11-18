//! Invoices (INV), price reductions and write-offs (CN, WO) and installment plans (PP).
//!
//! An invoice posts: Dr receivable (net, the patient) · Dr discounts · Cr service income (gross).
//! What an invoice still owes is never stored: net − reductions − write-offs − allocations.

use std::collections::HashMap;

use artaveo_shared::*;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::error::{CoreError, Result};

use super::ledger::*;
use super::util::*;

/// Lines on one invoice.
const MAX_LINES: usize = 100;
/// Installments in one plan.
const MAX_INSTALLMENTS: usize = 60;

// ───────────────────────────── amounts ─────────────────────────────

/// What settles an invoice, all posted documents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Settled {
    pub net: i64,
    pub reductions: i64,
    pub written_off: i64,
    pub allocated: i64,
}

impl Settled {
    pub fn remaining(&self) -> i64 {
        (self.net - self.reductions - self.written_off - self.allocated).max(0)
    }
}

/// SQL columns (on `fin_doc d`, an invoice) giving net, reductions, write-offs and allocations,
/// optionally only of documents written before document `?N` (see [`settled_before`]).
const SETTLED_COLS: &str = "d.amount,
    (SELECT COALESCE(SUM(x.amount), 0) FROM fin_doc x WHERE x.ref_id = d.id AND x.kind = 'cn' AND x.status = 'posted'),
    (SELECT COALESCE(SUM(x.amount), 0) FROM fin_doc x WHERE x.ref_id = d.id AND x.kind = 'wo' AND x.status = 'posted'),
    (SELECT COALESCE(SUM(l.amount), 0) FROM fin_line l JOIN fin_doc a ON a.id = l.doc_id
        WHERE l.ref_id = d.id AND a.kind IN ('rc', 'al') AND a.status = 'posted')";

fn settled_from(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<Settled> {
    Ok(Settled { net: r.get(at)?, reductions: r.get(at + 1)?, written_off: r.get(at + 2)?, allocated: r.get(at + 3)? })
}

pub(crate) fn settled(conn: &Connection, invoice_id: i64) -> Result<Settled> {
    Ok(conn.query_row(&format!("SELECT {SETTLED_COLS} FROM fin_doc d WHERE d.id = ?1"), [invoice_id], |r| {
        settled_from(r, 0)
    })?)
}

/// Allocations and reductions of an invoice made by documents older than `before` (a plan).
fn settled_before(conn: &Connection, invoice_id: i64, before: i64) -> Result<(i64, i64)> {
    let allocated = scalar(
        conn,
        "SELECT SUM(l.amount) FROM fin_line l JOIN fin_doc a ON a.id = l.doc_id
         WHERE l.ref_id = ?1 AND a.kind IN ('rc', 'al') AND a.status = 'posted' AND a.id < ?2",
        &[&invoice_id, &before],
    )?;
    let reduced = scalar(
        conn,
        "SELECT SUM(amount) FROM fin_doc WHERE ref_id = ?1 AND kind IN ('cn', 'wo') AND status = 'posted' AND id < ?2",
        &[&invoice_id, &before],
    )?;
    Ok((allocated, reduced))
}

pub(crate) fn pay_state(status: DocStatus, s: &Settled) -> PayState {
    if status != DocStatus::Posted {
        PayState::Void
    } else if s.remaining() == 0 {
        PayState::Paid
    } else if s.allocated > 0 || s.reductions > 0 {
        PayState::Partial
    } else {
        PayState::Unpaid
    }
}

// ───────────────────────────── reading ─────────────────────────────

const INVOICE_SELECT: &str = "SELECT d.id, d.year, d.serial, d.party_id, p.full_name, p.patient_number, d.status, d.doc_date,
    d.created_at, d.note, d.void_reason, d.print_count, d.created_by";

fn lines_of(conn: &Connection, invoice_id: i64) -> Result<Vec<InvoiceLineInfo>> {
    let mut stmt = conn.prepare_cached(
        "SELECT l.no, l.item_id, l.service_id, l.name, l.teeth, l.provider_id, dr.full_name, l.qty, l.unit_price,
            l.list_price, l.amount, l.discount
         FROM fin_line l LEFT JOIN doctor dr ON dr.id = l.provider_id WHERE l.doc_id = ?1 ORDER BY l.no",
    )?;
    let rows = stmt
        .query_map([invoice_id], |r| {
            let gross: i64 = r.get(10)?;
            let discount: i64 = r.get::<_, Option<i64>>(11)?.unwrap_or(0);
            Ok(InvoiceLineInfo {
                no: r.get(0)?,
                treatment_item_id: r.get(1)?,
                service_id: r.get(2)?,
                name: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                teeth: parse_teeth(&r.get::<_, Option<String>>(4)?.unwrap_or_default()),
                doctor_id: r.get(5)?,
                doctor_name: r.get(6)?,
                quantity: r.get::<_, Option<i64>>(7)?.unwrap_or(1),
                unit_price: r.get::<_, Option<i64>>(8)?.unwrap_or(gross),
                listed_price: r.get::<_, Option<i64>>(9)?.unwrap_or(0),
                gross,
                discount,
                net: gross - discount,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn adjustments_of(conn: &Connection, invoice_id: i64) -> Result<Vec<AdjustmentInfo>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, kind, year, serial, doc_date, amount, reason, status FROM fin_doc
         WHERE ref_id = ?1 AND kind IN ('cn', 'wo') ORDER BY id",
    )?;
    let rows = stmt
        .query_map([invoice_id], |r| {
            let kind: String = r.get(1)?;
            Ok(AdjustmentInfo {
                id: r.get(0)?,
                number: number_of(&kind, r.get(2)?, r.get(3)?),
                kind: if kind == "wo" { AdjustKind::WriteOff } else { AdjustKind::Reduction },
                date: iso_of(r.get(4)?),
                amount: r.get(5)?,
                reason: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                status: status_of(&r.get::<_, String>(7)?),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn allocations_of(conn: &Connection, invoice_id: i64) -> Result<Vec<AllocationInfo>> {
    let mut stmt = conn.prepare_cached(
        "SELECT a.id, a.kind, a.year, a.serial, a.doc_date, l.amount FROM fin_line l JOIN fin_doc a ON a.id = l.doc_id
         WHERE l.ref_id = ?1 AND a.kind IN ('rc', 'al') AND a.status = 'posted' ORDER BY a.id",
    )?;
    let rows = stmt
        .query_map([invoice_id], |r| {
            let kind: String = r.get(1)?;
            Ok(AllocationInfo {
                doc_id: r.get(0)?,
                number: number_of(&kind, r.get(2)?, r.get(3)?),
                kind: kind_of(&kind),
                date: iso_of(r.get(4)?),
                amount: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// The active plan of an invoice with each installment's state (from what settled it since).
pub(crate) fn plan_of(conn: &Connection, invoice_id: i64, now: &Settled) -> Result<Option<PlanInfo>> {
    let row: Option<(i64, i64, i64, i64, Option<String>)> = conn
        .query_row(
            "SELECT id, year, serial, doc_date, reason FROM fin_doc
             WHERE ref_id = ?1 AND kind = 'pp' AND status = 'posted' ORDER BY id DESC LIMIT 1",
            [invoice_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let Some((id, year, serial, date, reason)) = row else { return Ok(None) };
    let version = scalar(conn, "SELECT COUNT(*) FROM fin_doc WHERE ref_id = ?1 AND kind = 'pp' AND id <= ?2", &[
        &invoice_id,
        &id,
    ])?;
    let mut stmt = conn.prepare_cached("SELECT no, due_date, amount FROM fin_line WHERE doc_id = ?1 ORDER BY no")?;
    let items: Vec<(i64, i64, i64)> = stmt
        .query_map([id], |r| Ok((r.get(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0), r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let (alloc_base, reduce_base) = settled_before(conn, invoice_id, id)?;
    let paid_since = (now.allocated - alloc_base).max(0);
    let reduced_since = (now.reductions + now.written_off - reduce_base).max(0);
    Ok(Some(PlanInfo {
        id,
        number: format_number("PP", year, serial),
        version,
        date: iso_of(date),
        reason,
        installments: installment_states(&items, paid_since, reduced_since, today()),
    }))
}

/// Reductions after a plan shrink its installments from the last one back; money paid since the
/// plan covers them from the first one on (FR-04).
pub(crate) fn installment_states(
    items: &[(i64, i64, i64)],
    paid_since: i64,
    reduced_since: i64,
    today: i64,
) -> Vec<InstallmentInfo> {
    let mut amounts: Vec<i64> = items.iter().map(|i| i.2).collect();
    let mut cut = reduced_since;
    for a in amounts.iter_mut().rev() {
        let take = (*a).min(cut);
        *a -= take;
        cut -= take;
    }
    let mut left = paid_since;
    items
        .iter()
        .zip(amounts)
        .filter(|(_, amount)| *amount > 0)
        .map(|(&(no, due, _), amount)| {
            let paid = left.min(amount);
            left -= paid;
            let state = if paid >= amount {
                InstallmentState::Paid
            } else if due < today {
                InstallmentState::Overdue
            } else if paid > 0 {
                InstallmentState::Partial
            } else {
                InstallmentState::Pending
            };
            InstallmentInfo { no, due_date: iso_of(due), amount, paid, state }
        })
        .collect()
}

struct Head {
    id: i64,
    number: String,
    patient_id: String,
    patient_name: String,
    patient_number: String,
    status: DocStatus,
    date: i64,
    created_at: i64,
    note: Option<String>,
    void_reason: Option<String>,
    print_count: i64,
    created_by: Option<String>,
    settled: Settled,
}

fn head_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Head> {
    Ok(Head {
        id: r.get(0)?,
        number: format_number("INV", r.get(1)?, r.get(2)?),
        patient_id: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        patient_name: r.get(4)?,
        patient_number: r.get(5)?,
        status: status_of(&r.get::<_, String>(6)?),
        date: r.get(7)?,
        created_at: r.get(8)?,
        note: r.get(9)?,
        void_reason: r.get(10)?,
        print_count: r.get(11)?,
        created_by: r.get(12)?,
        settled: settled_from(r, 13)?,
    })
}

fn finish(conn: &Connection, h: Head) -> Result<InvoiceInfo> {
    let lines = lines_of(conn, h.id)?;
    let gross: i64 = lines.iter().map(|l| l.gross).sum();
    let s = h.settled;
    Ok(InvoiceInfo {
        plan: if h.status == DocStatus::Posted { plan_of(conn, h.id, &s)? } else { None },
        adjustments: adjustments_of(conn, h.id)?,
        allocations: allocations_of(conn, h.id)?,
        pay_state: pay_state(h.status, &s),
        gross,
        discount: gross - s.net,
        net: s.net,
        reductions: s.reductions,
        written_off: s.written_off,
        allocated: s.allocated,
        remaining: if h.status == DocStatus::Posted { s.remaining() } else { 0 },
        lines,
        created_by_name: user_name(conn, &h.created_by)?,
        id: h.id,
        number: h.number,
        patient_id: h.patient_id,
        patient_name: h.patient_name,
        patient_number: h.patient_number,
        status: h.status,
        date: iso_of(h.date),
        created_at: iso_at(h.created_at),
        note: h.note,
        void_reason: h.void_reason,
        print_count: h.print_count,
    })
}

pub fn get(conn: &Connection, id: i64) -> Result<InvoiceInfo> {
    let h = conn
        .query_row(
            &format!(
                "{INVOICE_SELECT}, {SETTLED_COLS} FROM fin_doc d JOIN patient p ON p.id = d.party_id
                 WHERE d.id = ?1 AND d.kind = 'inv'"
            ),
            [id],
            head_row,
        )
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("invoice {id}"))
                .on_field("invoice_id", ValidationRule::InvoiceNotFound)
        })?;
    finish(conn, h)
}

pub fn list(conn: &Connection, p: &InvoiceListParams) -> Result<Vec<InvoiceInfo>> {
    let mut filter = String::new();
    let mut args: Vec<Value> = Vec::new();
    if let Some(pid) = p.patient_id.as_deref().filter(|s| !s.is_empty()) {
        filter.push_str(" AND d.party_id = ?");
        args.push(Value::Text(pid.to_string()));
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
            " AND ({NUMBER_SQL} LIKE ? OR p.patient_number LIKE ? OR p.full_name LIKE ? OR COALESCE(p.phone, '') LIKE ?)"
        ));
        for _ in 0..4 {
            args.push(Value::Text(like.clone()));
        }
    }
    if p.open_only {
        filter.push_str(" AND d.status = 'posted'");
    }
    let limit = p.limit.unwrap_or(100).min(MAX_LIST) as usize;
    let offset = p.offset.unwrap_or(0) as usize;
    let sql = format!(
        "{INVOICE_SELECT}, {SETTLED_COLS} FROM fin_doc d JOIN patient p ON p.id = d.party_id
         WHERE d.kind = 'inv'{filter} ORDER BY d.id DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(args.iter()))?;
    let mut out = Vec::new();
    let mut skipped = 0;
    while let Some(row) = rows.next()? {
        let h = head_row(row)?;
        if p.open_only && h.settled.remaining() == 0 {
            continue;
        }
        if skipped < offset {
            skipped += 1;
            continue;
        }
        out.push(finish(conn, h)?);
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

// ───────────────────────────── billable treatments ─────────────────────────────

/// SQL test (on `treatment_item i`) for a treatment that may be invoiced: completed, or (CT-08) a
/// multi-session course such as orthodontics that has started.
const BILLABLE_ITEM: &str = "(i.status = 'completed'
    OR (i.status = 'in_progress' AND i.sessions_planned > 1
        AND EXISTS(SELECT 1 FROM treatment_session s WHERE s.item_id = i.id)))";

/// Completed treatments of the patient not on any invoice yet (and started multi-session courses).
pub fn billable(conn: &Connection, patient_id: &str) -> Result<Vec<BillableItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT i.id, i.service_id, s.name_fa, s.name_ps, s.name_en, i.teeth, i.surfaces, i.units, i.unit_price,
            i.doctor_id, i.completed_at, i.details, i.status = 'completed', i.sessions_planned,
            (SELECT COUNT(*) FROM treatment_session x WHERE x.item_id = i.id)
         FROM treatment_item i JOIN service s ON s.id = i.service_id
         WHERE i.patient_id = ?1 AND {BILLABLE_ITEM} AND i.billed_at IS NULL AND i.deleted_at IS NULL
         ORDER BY COALESCE(i.completed_at, i.created_at), i.sort_order"
    ))?;
    let rows = stmt
        .query_map([patient_id], |r| {
            Ok(BillableItem {
                item_id: r.get(0)?,
                service_id: r.get(1)?,
                names: trio(r.get(2)?, r.get(3)?, r.get(4)?),
                teeth: parse_teeth(&r.get::<_, String>(5)?),
                surfaces: r.get(6)?,
                units: r.get(7)?,
                unit_price: r.get(8)?,
                doctor_id: r.get(9)?,
                completed_at: r.get(10)?,
                details: r.get(11)?,
                completed: r.get(12)?,
                sessions_planned: r.get(13)?,
                sessions_done: r.get(14)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// The service's name in the clinic's data language (frozen on the invoice line) and its price.
fn service_name(conn: &Connection, service_id: &str) -> Result<(String, i64)> {
    let (names, price) = conn
        .query_row(
            "SELECT name_fa, name_ps, name_en, price FROM service WHERE id = ?1 AND deleted_at IS NULL",
            [service_id],
            |r| Ok((trio(r.get(0)?, r.get(1)?, r.get(2)?), r.get::<_, i64>(3)?)),
        )
        .optional()?
        .ok_or_else(|| CoreError::invalid("service_id", ValidationRule::ServiceNotFound, "unknown service"))?;
    Ok((pick_name(conn, &names)?, price))
}

pub(crate) fn pick_name(conn: &Connection, t: &Translations) -> Result<String> {
    let lang = crate::clinic::get(conn)?.default_language;
    let first = match lang {
        Language::Ps => &t.ps,
        Language::En => &t.en,
        Language::Fa => &t.fa,
    };
    Ok([first, &t.fa, &t.ps, &t.en].into_iter().find(|s| !s.trim().is_empty()).cloned().unwrap_or_default())
}

// ───────────────────────────── writing ─────────────────────────────

/// `(patient, service, teeth, doctor, unit price, units, billable now, billed at)` of a treatment.
type ItemRow = (String, String, String, Option<String>, i64, i64, bool, Option<String>);

struct NewLine {
    item_id: Option<String>,
    service_id: Option<String>,
    name: String,
    teeth: String,
    doctor_id: Option<String>,
    quantity: i64,
    unit_price: i64,
    listed_price: i64,
    discount: i64,
}

impl NewLine {
    fn gross(&self) -> i64 {
        self.quantity * self.unit_price
    }
}

/// Shares `total` over `weights` in proportion, the remainder going to the largest fractions so the
/// parts add up to exactly `total` (DESIGN ب۱۱.۴).
pub(crate) fn share_out(total: i64, weights: &[i64]) -> Vec<i64> {
    let sum: i128 = weights.iter().map(|&w| w.max(0) as i128).sum();
    if sum == 0 || total == 0 {
        return vec![0; weights.len()];
    }
    let mut parts: Vec<(usize, i64, i128)> = weights
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            let exact = total as i128 * w.max(0) as i128;
            (i, (exact / sum) as i64, exact % sum)
        })
        .collect();
    let mut left = total - parts.iter().map(|p| p.1).sum::<i64>();
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by(|&a, &b| parts[b].2.cmp(&parts[a].2).then(a.cmp(&b)));
    for i in order {
        if left == 0 {
            break;
        }
        parts[i].1 += 1;
        left -= 1;
    }
    parts.into_iter().map(|p| p.1).collect()
}

pub fn create(conn: &Connection, actor: &Actor, can_override: bool, p: &CreateInvoiceParams) -> Result<InvoiceInfo> {
    check_patient(conn, &p.patient_id)?;
    if p.items.is_empty() && p.extra_lines.is_empty() {
        return Err(CoreError::invalid("items", ValidationRule::InvoiceEmpty, "an invoice needs at least one line"));
    }
    if p.items.len() + p.extra_lines.len() > MAX_LINES {
        return Err(CoreError::invalid("items", ValidationRule::InvoiceEmpty, "at most 100 lines"));
    }
    let mut lines: Vec<NewLine> = Vec::new();
    let mut overrides = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for pick in &p.items {
        if !seen.insert(pick.item_id.clone()) {
            continue;
        }
        let row: Option<ItemRow> = conn
            .query_row(
                &format!(
                    "SELECT patient_id, service_id, teeth, doctor_id, unit_price, units, {BILLABLE_ITEM}, billed_at
                     FROM treatment_item i WHERE id = ?1 AND deleted_at IS NULL"
                ),
                [&pick.item_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
            )
            .optional()?;
        let (patient, service, teeth, doctor, price, units, billable_now, billed) =
            row.ok_or_else(|| CoreError::invalid("items", ValidationRule::ItemNotBillable, "unknown treatment"))?;
        if patient != p.patient_id || !billable_now {
            return Err(CoreError::invalid("items", ValidationRule::ItemNotBillable, "this treatment cannot be invoiced"));
        }
        if billed.is_some() {
            return Err(CoreError::invalid("items", ValidationRule::ItemAlreadyBilled, "this treatment is already invoiced"));
        }
        let (name, _) = service_name(conn, &service)?;
        let unit_price = money("unit_price", pick.unit_price.unwrap_or(price), true)?;
        if unit_price != price {
            if !can_override {
                return Err(CoreError::forbidden(crate::auth::perm::BILLING_OVERRIDE));
            }
            overrides.push(json!({"item_id": pick.item_id, "from": price, "to": unit_price}));
        }
        lines.push(NewLine {
            item_id: Some(pick.item_id.clone()),
            service_id: Some(service),
            name,
            teeth,
            doctor_id: doctor,
            quantity: units.max(1),
            unit_price,
            listed_price: price,
            discount: money("discount", pick.discount, true)?,
        });
    }
    for x in &p.extra_lines {
        if !(1..=1000).contains(&x.quantity) {
            return Err(CoreError::invalid("quantity", ValidationRule::BillingAmount, "quantity is 1 to 1000"));
        }
        let unit_price = money("unit_price", x.unit_price, true)?;
        let doctor_id = check_doctor(conn, &x.doctor_id)?;
        let (name, listed, service_id) = match x.service_id.as_deref().filter(|s| !s.is_empty()) {
            Some(sid) => {
                let (name, price) = service_name(conn, sid)?;
                if unit_price != price {
                    if !can_override {
                        return Err(CoreError::forbidden(crate::auth::perm::BILLING_OVERRIDE));
                    }
                    overrides.push(json!({"service_id": sid, "from": price, "to": unit_price}));
                }
                (name, price, Some(sid.to_string()))
            }
            None => (required("description", x.description.as_deref().unwrap_or(""), 120)?, 0, None),
        };
        lines.push(NewLine {
            item_id: None,
            service_id,
            name,
            teeth: teeth_text(&x.teeth),
            doctor_id,
            quantity: x.quantity,
            unit_price,
            listed_price: listed,
            discount: money("discount", x.discount, true)?,
        });
    }
    let gross: i64 = lines.iter().map(NewLine::gross).sum();
    if gross > MAX_AMOUNT {
        return Err(CoreError::invalid("items", ValidationRule::BillingAmount, "the invoice total is too large"));
    }
    if lines.iter().any(|l| l.discount > l.gross()) {
        return Err(CoreError::invalid("discount", ValidationRule::DiscountRange, "a line discount is more than the line"));
    }
    let whole = money("discount", p.discount, true)?;
    let after_lines: Vec<i64> = lines.iter().map(|l| l.gross() - l.discount).collect();
    if whole > after_lines.iter().sum::<i64>() {
        return Err(CoreError::invalid("discount", ValidationRule::DiscountRange, "the discount is more than the total"));
    }
    for (l, share) in lines.iter_mut().zip(share_out(whole, &after_lines)) {
        l.discount += share;
    }
    let discount: i64 = lines.iter().map(|l| l.discount).sum();
    let net = gross - discount;
    let note = optional("note", &p.note, 500)?;
    let date = today();
    let (id, number) = insert_doc(conn, actor, &NewDoc {
        kind: Some(DocKind::Inv),
        date,
        patient_id: Some(&p.patient_id),
        amount: net,
        note: note.as_deref(),
        ..Default::default()
    })?;
    for (i, l) in lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO fin_line(doc_id, no, account_id, amount, qty, unit_price, list_price, discount, item_id, service_id,
                provider_id, teeth, name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id,
                i as i64 + 1,
                INCOME,
                l.gross(),
                l.quantity,
                l.unit_price,
                l.listed_price,
                l.discount,
                l.item_id,
                l.service_id,
                l.doctor_id,
                Some(l.teeth.as_str()).filter(|t| !t.is_empty()),
                l.name
            ],
        )?;
        if let Some(item) = &l.item_id {
            conn.execute("UPDATE treatment_item SET billed_at = ?1 WHERE id = ?2", params![crate::clock::now_iso(), item])?;
        }
    }
    post(conn, id, date, vec![dr(RECEIVABLE, net).of(&p.patient_id), dr(DISCOUNTS, discount), cr(INCOME, gross)])?;
    audit::record(
        conn,
        actor,
        "invoice.create",
        Some("patient"),
        Some(&p.patient_id),
        None,
        Some(&json!({"id": id, "number": number, "net": net, "discount": discount, "price_overrides": overrides})),
    )?;
    if p.use_deposit && net > 0 {
        let deposit = super::receipts::deposit(conn, &p.patient_id, None)?;
        if deposit > 0 {
            super::receipts::allocate(conn, actor, &AllocateParams {
                patient_id: p.patient_id.clone(),
                allocations: vec![NewAllocation { invoice_id: id, amount: deposit.min(net) }],
                reason: None,
            })?;
        }
    }
    if !p.installments.is_empty() {
        write_plan(conn, actor, id, &p.installments, None)?;
    }
    get(conn, id)
}

fn posted_invoice(conn: &Connection, id: i64) -> Result<DocHead> {
    let h = head(conn, id).map_err(|e| e.on_field("invoice_id", ValidationRule::InvoiceNotFound))?;
    if h.kind != DocKind::Inv {
        return Err(CoreError::invalid("invoice_id", ValidationRule::InvoiceNotFound, "not an invoice"));
    }
    if h.status != DocStatus::Posted {
        return Err(CoreError::invalid("invoice_id", ValidationRule::InvoiceVoid, "this invoice was voided"));
    }
    Ok(h)
}

/// A price reduction (CN: Dr reductions, Cr receivable) or a write-off (WO: Dr bad debts, Cr receivable).
pub fn adjust(conn: &Connection, actor: &Actor, p: &AdjustInvoiceParams) -> Result<InvoiceInfo> {
    let inv = posted_invoice(conn, p.invoice_id)?;
    let patient = inv.party_id.clone().unwrap_or_default();
    let amount = money("amount", p.amount, false)?;
    let reason = required("reason", &p.reason, 300)?;
    if amount > settled(conn, inv.id)?.remaining() {
        return Err(CoreError::invalid(
            "amount",
            ValidationRule::CreditNoteExceeds,
            "more than what is still owed on the invoice",
        ));
    }
    let (kind, account) = match p.kind {
        AdjustKind::Reduction => (DocKind::Cn, REDUCTIONS),
        AdjustKind::WriteOff => (DocKind::Wo, BAD_DEBT),
    };
    let date = today();
    let (id, number) = insert_doc(conn, actor, &NewDoc {
        kind: Some(kind),
        date,
        patient_id: Some(&patient),
        ref_id: Some(inv.id),
        amount,
        reason: Some(&reason),
        ..Default::default()
    })?;
    post(conn, id, date, vec![dr(account, amount), cr(RECEIVABLE, amount).of(&patient)])?;
    audit::record(
        conn,
        actor,
        if kind == DocKind::Wo { "invoice.write_off" } else { "invoice.reduction" },
        Some("patient"),
        Some(&patient),
        Some(&json!({"invoice": inv.number})),
        Some(&json!({"number": number, "amount": amount, "reason": reason})),
    )?;
    get(conn, inv.id)
}

fn write_plan(
    conn: &Connection,
    actor: &Actor,
    invoice_id: i64,
    list: &[NewInstallment],
    reason: Option<&str>,
) -> Result<()> {
    let inv = posted_invoice(conn, invoice_id)?;
    let patient = inv.party_id.clone().unwrap_or_default();
    let remaining = settled(conn, invoice_id)?.remaining();
    let oldest = days_after(today(), -365);
    let mut rows = Vec::with_capacity(list.len());
    for x in list {
        let due = ymd(&date_of("due_date", &x.due_date)?);
        // CT-09: a late installment is allowed (an old debt being scheduled), a year back is a typo.
        if due < oldest {
            return Err(CoreError::invalid(
                "installments",
                ValidationRule::InstallmentsInvalid,
                "a due date is more than a year in the past",
            ));
        }
        rows.push((due, money("amount", x.amount, false)?));
    }
    rows.sort();
    let total: i64 = rows.iter().map(|r| r.1).sum();
    if rows.len() > MAX_INSTALLMENTS || total != remaining || remaining == 0 {
        return Err(CoreError::invalid(
            "installments",
            ValidationRule::InstallmentsInvalid,
            "the installments must add up to what is still owed",
        ));
    }
    let (id, number) = insert_doc(conn, actor, &NewDoc {
        kind: Some(DocKind::Pp),
        date: today(),
        patient_id: Some(&patient),
        ref_id: Some(invoice_id),
        amount: total,
        reason,
        ..Default::default()
    })?;
    for (i, (due, amount)) in rows.iter().enumerate() {
        conn.execute(
            "INSERT INTO fin_line(doc_id, no, amount, due_date) VALUES (?1, ?2, ?3, ?4)",
            params![id, i as i64 + 1, amount, due],
        )?;
    }
    audit::record(
        conn,
        actor,
        "invoice.plan",
        Some("patient"),
        Some(&patient),
        Some(&json!({"invoice": inv.number})),
        Some(&json!({"number": number, "count": rows.len(), "total": total, "reason": reason})),
    )?;
    Ok(())
}

/// `yyyymmdd` moved by a number of days.
pub(crate) fn days_after(day: i64, days: i64) -> i64 {
    crate::scheduling::parse_date("day", &iso_of(day))
        .map(|d| ymd(&crate::scheduling::format_date(d + time::Duration::days(days))))
        .unwrap_or(day)
}

/// Replaces the invoice's installment plan (the old one stays, marked superseded); empty = no plan.
pub fn set_plan(conn: &Connection, actor: &Actor, p: &SetPlanParams) -> Result<InvoiceInfo> {
    posted_invoice(conn, p.invoice_id)?;
    let current: Option<i64> = conn
        .query_row(
            "SELECT id FROM fin_doc WHERE ref_id = ?1 AND kind = 'pp' AND status = 'posted' ORDER BY id DESC LIMIT 1",
            [p.invoice_id],
            |r| r.get(0),
        )
        .optional()?;
    let reason = optional("reason", &p.reason, 300)?;
    if let Some(old) = current {
        let reason = reason.as_deref().ok_or_else(|| {
            CoreError::invalid("reason", ValidationRule::Required, "say why the installment plan changes")
        })?;
        conn.execute(
            "UPDATE fin_doc SET status = 'superseded', void_reason = ?1, voided_at = ?2, voided_by = ?3 WHERE id = ?4",
            params![reason, now_secs(), actor.user_id, old],
        )?;
    }
    if !p.installments.is_empty() {
        write_plan(conn, actor, p.invoice_id, &p.installments, reason.as_deref())?;
    }
    get(conn, p.invoice_id)
}

/// Voids an invoice: nothing may be allocated to it and no reduction or write-off may stand.
/// The treatments go back to "ready for an invoice"; an active plan goes with the invoice.
pub(crate) fn void(conn: &Connection, actor: &Actor, h: &DocHead, reason: &str) -> Result<()> {
    let s = settled(conn, h.id)?;
    if s.allocated != 0 {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::InvoiceHasPayments,
            "take the money on this invoice back to the deposit before voiding it",
        ));
    }
    if s.reductions != 0 || s.written_off != 0 {
        return Err(CoreError::invalid("id", ValidationRule::DocNotVoidable, "void the reductions of this invoice first"));
    }
    void_doc(conn, actor, h, reason)?;
    conn.execute(
        "UPDATE fin_doc SET status = 'void', void_reason = ?1, voided_at = ?2, voided_by = ?3
         WHERE ref_id = ?4 AND kind = 'pp' AND status = 'posted'",
        params![reason, now_secs(), actor.user_id, h.id],
    )?;
    conn.execute(
        "UPDATE treatment_item SET billed_at = NULL
         WHERE id IN (SELECT item_id FROM fin_line WHERE doc_id = ?1 AND item_id IS NOT NULL)",
        [h.id],
    )?;
    Ok(())
}

/// Invoice lines of many invoices at once: `invoice id → [(provider, service, name, qty, net)]`.
pub(crate) type LineNet = (Option<String>, Option<String>, String, i64, i64);

pub(crate) fn line_nets(conn: &Connection, ids: &[i64]) -> Result<HashMap<i64, Vec<LineNet>>> {
    let mut out: HashMap<i64, Vec<LineNet>> = HashMap::new();
    let mut stmt = conn.prepare_cached(
        "SELECT provider_id, service_id, COALESCE(name, ''), COALESCE(qty, 1), amount - COALESCE(discount, 0)
         FROM fin_line WHERE doc_id = ?1 ORDER BY no",
    )?;
    for &id in ids {
        let rows: Vec<LineNet> = stmt
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        out.insert(id, rows);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_out_spreads_the_remainder_and_adds_up() {
        assert_eq!(share_out(300, &[2000, 300]), vec![261, 39]);
        assert_eq!(share_out(100, &[1, 1, 1]), vec![34, 33, 33]);
        assert_eq!(share_out(350_000, &[1; 12]).iter().sum::<i64>(), 350_000);
        assert_eq!(share_out(0, &[5, 5]), vec![0, 0]);
        assert_eq!(share_out(10, &[0, 0]), vec![0, 0]);
    }

    #[test]
    fn reductions_shrink_the_last_installments_and_payments_cover_the_first() {
        // Three installments of 1000 (due 1 Jan, 1 Feb, 1 Mar); 1200 paid, 1500 taken off the price.
        let items = [(1, 20260101, 1000), (2, 20260201, 1000), (3, 20260301, 1000)];
        let s = installment_states(&items, 1200, 1500, 20260215);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].amount, s[0].paid, s[0].state), (1000, 1000, InstallmentState::Paid));
        assert_eq!((s[1].amount, s[1].paid, s[1].state), (500, 200, InstallmentState::Overdue));
        let s = installment_states(&items, 0, 0, 20251201);
        assert!(s.iter().all(|i| i.state == InstallmentState::Pending));
    }
}
