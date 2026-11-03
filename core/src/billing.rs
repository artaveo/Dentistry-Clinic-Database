//! Phase 6 — billing (roadmap 6.1–6.8): invoices made from treatments done, payments (partial, advance,
//! credit, refund), installment plans, the patient's balance and statement, doctor commission, clinic
//! expenses and the daily cash closing. Money is AFN × 100 (ADR-06). An issued invoice and every payment
//! row are never edited (ADR-11): a mistake is a void, a credit note or a refund, each with its reason
//! and an audit entry. Document numbers (INV / RC / CU / RV / CN) are taken inside the writing
//! transaction, so none is skipped or repeated.

use std::collections::{BTreeMap, HashMap};

use artaveo_shared::*;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::clock::{local_now, now_iso, today_iso};
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::formulary::clean_translations;
use crate::ids::new_id;
use crate::scheduling::{format_date, parse_date};

/// Highest amount accepted: 1 000 000 000 AFN (in AFN × 100).
const MAX_AMOUNT: i64 = 100_000_000_000;
const MAX_LIST: u32 = 300;
/// Lines of an invoice shown on its receipts.
const RECEIPT_LINES: usize = 12;

// ───────────────────────────── small helpers ─────────────────────────────

fn money(field: &str, v: i64, allow_zero: bool) -> Result<i64> {
    let ok = if allow_zero { v >= 0 } else { v > 0 };
    if !ok || v > MAX_AMOUNT {
        return Err(CoreError::invalid(
            field,
            ValidationRule::BillingAmount,
            format!("{field} is not a valid amount"),
        ));
    }
    Ok(v)
}

fn required(field: &str, s: &str, max: usize) -> Result<String> {
    let s = s.trim();
    if s.is_empty() {
        return Err(CoreError::invalid(field, ValidationRule::Required, format!("{field} is required")));
    }
    if s.chars().count() > max {
        return Err(CoreError::invalid(
            field,
            ValidationRule::DocumentText,
            format!("{field} is at most {max} characters"),
        ));
    }
    Ok(s.to_string())
}

fn optional(field: &str, s: &Option<String>, max: usize) -> Result<Option<String>> {
    match s.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(v) => required(field, v, max).map(Some),
    }
}

fn date_of(field: &'static str, s: &str) -> Result<String> {
    Ok(format_date(parse_date(field, s.trim())?))
}

fn opt_date(field: &'static str, s: &Option<String>) -> Result<Option<String>> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| date_of(field, s)).transpose()
}

fn scalar(conn: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> Result<i64> {
    Ok(conn.query_row(sql, args, |r| r.get::<_, Option<i64>>(0))?.unwrap_or(0))
}

fn next_number(conn: &Connection, prefix: &str) -> Result<String> {
    let year = crate::documents::shamsi_year_today();
    conn.execute(
        "INSERT INTO document_counter(prefix, year, value) VALUES (?1, ?2, 1)
         ON CONFLICT(prefix, year) DO UPDATE SET value = value + 1",
        params![prefix, year],
    )?;
    let n: i64 = conn.query_row(
        "SELECT value FROM document_counter WHERE prefix = ?1 AND year = ?2",
        params![prefix, year],
        |r| r.get(0),
    )?;
    Ok(format!("{prefix}-{year}-{n:06}"))
}

fn parse_teeth(s: &str) -> Vec<u8> {
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

fn trio(fa: String, ps: String, en: String) -> Translations {
    Translations { fa, ps, en }
}

fn setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

fn set_setting(conn: &Connection, actor: &Actor, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO setting(key, value, updated_at, updated_by) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at, updated_by = excluded.updated_by",
        params![key, value, now_iso(), actor.user_id],
    )?;
    Ok(())
}

fn check_patient(conn: &Connection, patient_id: &str) -> Result<()> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT merged_into_id FROM patient WHERE id = ?1 AND deleted_at IS NULL",
            [patient_id],
            |r| r.get(0),
        )
        .optional()?;
    match row {
        None => Err(CoreError::invalid("patient_id", ValidationRule::PatientNotFound, "unknown patient")),
        Some(Some(_)) => Err(CoreError::invalid(
            "patient_id",
            ValidationRule::PatientMerged,
            "this record was merged into another patient",
        )),
        Some(None) => Ok(()),
    }
}

// ───────────────────────────── payment methods ─────────────────────────────

const METHOD_SELECT: &str =
    "SELECT id, code, name_fa, name_ps, name_en, is_cash, sort_order, is_system, is_active, version
    FROM payment_method WHERE deleted_at IS NULL";

fn map_method(r: &rusqlite::Row<'_>) -> rusqlite::Result<PaymentMethodInfo> {
    Ok(PaymentMethodInfo {
        id: r.get(0)?,
        code: r.get(1)?,
        name: trio(r.get(2)?, r.get(3)?, r.get(4)?),
        is_cash: r.get::<_, i64>(5)? == 1,
        sort_order: r.get(6)?,
        is_system: r.get::<_, i64>(7)? == 1,
        is_active: r.get::<_, i64>(8)? == 1,
        version: r.get(9)?,
    })
}

pub fn list_methods(conn: &Connection, include_inactive: bool) -> Result<Vec<PaymentMethodInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{METHOD_SELECT} {} ORDER BY sort_order, name_en",
        if include_inactive { "" } else { "AND is_active = 1" }
    ))?;
    let rows = stmt.query_map([], map_method)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn get_method(conn: &Connection, id: &str) -> Result<PaymentMethodInfo> {
    conn.query_row(&format!("{METHOD_SELECT} AND id = ?1"), [id], map_method).optional()?.ok_or_else(|| {
        CoreError::invalid("method_id", ValidationRule::PaymentMethodNotFound, "unknown payment method")
    })
}

pub fn save_method(
    conn: &Connection,
    actor: &Actor,
    p: &SavePaymentMethodParams,
) -> Result<PaymentMethodInfo> {
    let name = clean_translations("name", &p.name, 60)?;
    let now = now_iso();
    match p.id.as_deref().filter(|s| !s.is_empty()) {
        None => {
            let id = new_id();
            let order = scalar(conn, "SELECT MAX(sort_order) FROM payment_method", &[])? + 10;
            conn.execute(
                "INSERT INTO payment_method(id, code, name_fa, name_ps, name_en, is_cash, sort_order, is_active, created_at,
                    created_by, updated_at, updated_by)
                 VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?8, ?9)",
                params![id, name.fa, name.ps, name.en, p.is_cash, order, p.is_active, now, actor.user_id],
            )?;
            let after = get_method(conn, &id)?;
            audit::record(
                conn,
                actor,
                "payment_method.create",
                Some("payment_method"),
                Some(&id),
                None,
                Some(&json!(after.name)),
            )?;
            Ok(after)
        }
        Some(id) => {
            let before = get_method(conn, id)?;
            let cash = before.is_system && before.is_cash;
            if before.is_system && before.code == "cash" && !p.is_active {
                return Err(CoreError::invalid(
                    "is_active",
                    ValidationRule::InvalidParams,
                    "cash cannot be turned off",
                ));
            }
            let version = p.version.unwrap_or(before.version);
            let changed = conn.execute(
                "UPDATE payment_method SET name_fa = ?1, name_ps = ?2, name_en = ?3, is_cash = ?4, is_active = ?5,
                    updated_at = ?6, updated_by = ?7, version = version + 1
                 WHERE id = ?8 AND version = ?9 AND deleted_at IS NULL",
                params![name.fa, name.ps, name.en, if before.is_system { cash } else { p.is_cash }, p.is_active, now, actor.user_id, id, version],
            )?;
            expect_one_row(changed, "payment method")?;
            let after = get_method(conn, id)?;
            audit::record(
                conn,
                actor,
                "payment_method.update",
                Some("payment_method"),
                Some(id),
                Some(&json!(before)),
                Some(&json!(after)),
            )?;
            Ok(after)
        }
    }
}

// ───────────────────────────── amounts of an invoice ─────────────────────────────

/// Five columns every query over invoices needs: sum of lines, discount, credit notes, received, refunded.
const AMOUNT_COLS: &str = "(SELECT COALESCE(SUM(line_total), 0) FROM invoice_line WHERE invoice_id = i.id),
    i.discount,
    (SELECT COALESCE(SUM(amount), 0) FROM credit_note WHERE invoice_id = i.id),
    (SELECT COALESCE(SUM(amount), 0) FROM payment WHERE invoice_id = i.id AND kind IN ('payment', 'credit')),
    (SELECT COALESCE(SUM(amount), 0) FROM payment WHERE invoice_id = i.id AND kind = 'refund')";

#[derive(Debug, Clone, Copy, Default)]
struct Amounts {
    subtotal: i64,
    discount: i64,
    credit_notes: i64,
    paid: i64,
    refunded: i64,
}

impl Amounts {
    fn from_row(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<Self> {
        Ok(Amounts {
            subtotal: r.get(at)?,
            discount: r.get(at + 1)?,
            credit_notes: r.get(at + 2)?,
            paid: r.get(at + 3)?,
            refunded: r.get(at + 4)?,
        })
    }
    fn net(&self) -> i64 {
        (self.subtotal - self.discount - self.credit_notes).max(0)
    }
    fn applied(&self) -> i64 {
        (self.paid - self.refunded).max(0)
    }
    fn remaining(&self) -> i64 {
        (self.net() - self.applied()).max(0)
    }
}

fn amounts(conn: &Connection, invoice_id: &str) -> Result<Amounts> {
    Ok(conn.query_row(
        &format!("SELECT {AMOUNT_COLS} FROM invoice i WHERE i.id = ?1"),
        [invoice_id],
        |r| Amounts::from_row(r, 0),
    )?)
}

/// The patient's credit: advance payments not yet put on an invoice.
fn patient_credit(conn: &Connection, patient_id: &str) -> Result<i64> {
    scalar(
        conn,
        "SELECT COALESCE(SUM(CASE
                WHEN kind = 'payment' AND invoice_id IS NULL THEN amount
                WHEN kind = 'credit' THEN -amount
                WHEN kind = 'refund' AND returns_to_credit = 1 THEN amount
                WHEN kind = 'refund' AND invoice_id IS NULL THEN -amount
                ELSE 0 END), 0)
         FROM payment WHERE patient_id = ?1",
        &[&patient_id],
    )
}

// ───────────────────────────── invoices: reading ─────────────────────────────

const INVOICE_SELECT: &str =
    "SELECT i.id, i.number, i.patient_id, p.full_name, p.patient_number, i.status, i.issued_at,
    i.local_date, i.note, i.void_reason, i.print_count, u.display_name, i.installment_base, i.version
    FROM invoice i JOIN patient p ON p.id = i.patient_id LEFT JOIN app_user u ON u.id = i.created_by
    WHERE i.deleted_at IS NULL";

fn lines_of(conn: &Connection, invoice_id: &str) -> Result<Vec<InvoiceLineInfo>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, treatment_item_id, service_id, names_json, teeth, surfaces, doctor_id, quantity, unit_price,
            listed_price, line_total
         FROM invoice_line WHERE invoice_id = ?1 ORDER BY sort_order, rowid",
    )?;
    let rows: Vec<(InvoiceLineInfo, String)> = stmt
        .query_map([invoice_id], |r| {
            Ok((
                InvoiceLineInfo {
                    id: r.get(0)?,
                    treatment_item_id: r.get(1)?,
                    service_id: r.get(2)?,
                    names: Translations::default(),
                    teeth: parse_teeth(&r.get::<_, String>(4)?),
                    surfaces: r.get(5)?,
                    doctor_id: r.get(6)?,
                    quantity: r.get(7)?,
                    unit_price: r.get(8)?,
                    listed_price: r.get(9)?,
                    line_total: r.get(10)?,
                },
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    rows.into_iter()
        .map(|(mut l, names)| {
            l.names = serde_json::from_str(&names)?;
            Ok(l)
        })
        .collect()
}

fn credit_notes_of(conn: &Connection, invoice_id: &str) -> Result<Vec<CreditNoteInfo>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, number, amount, reason, issued_at FROM credit_note WHERE invoice_id = ?1 ORDER BY issued_at",
    )?;
    let rows = stmt
        .query_map([invoice_id], |r| {
            Ok(CreditNoteInfo {
                id: r.get(0)?,
                number: r.get(1)?,
                amount: r.get(2)?,
                reason: r.get(3)?,
                issued_at: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// The plan's installments, each covered by what the invoice received since the plan was set,
/// oldest first.
fn installments_of(
    conn: &Connection,
    invoice_id: &str,
    base: i64,
    applied: i64,
) -> Result<Vec<InstallmentInfo>> {
    let today = today_iso();
    let mut stmt = conn
        .prepare_cached("SELECT no, due_date, amount FROM installment WHERE invoice_id = ?1 ORDER BY no")?;
    let rows: Vec<(i64, String, i64)> = stmt
        .query_map([invoice_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut left = (applied - base).max(0);
    Ok(rows
        .into_iter()
        .map(|(no, due_date, amount)| {
            let paid = left.min(amount);
            left -= paid;
            let state = if paid >= amount {
                InstallmentState::Paid
            } else if due_date < today {
                InstallmentState::Overdue
            } else if paid > 0 {
                InstallmentState::Partial
            } else {
                InstallmentState::Pending
            };
            InstallmentInfo { no, due_date, amount, paid, state }
        })
        .collect())
}

fn pay_state(status: InvoiceStatus, a: &Amounts) -> PayState {
    if status == InvoiceStatus::Void {
        PayState::Void
    } else if a.remaining() == 0 {
        PayState::Paid
    } else if a.applied() > 0 {
        PayState::Partial
    } else {
        PayState::Unpaid
    }
}

fn status_from(s: &str) -> InvoiceStatus {
    if s == "void" {
        InvoiceStatus::Void
    } else {
        InvoiceStatus::Issued
    }
}

fn hydrate(conn: &Connection, r: &rusqlite::Row<'_>) -> rusqlite::Result<Raw> {
    Ok(Raw {
        id: r.get(0)?,
        number: r.get(1)?,
        patient_id: r.get(2)?,
        patient_name: r.get(3)?,
        patient_number: r.get(4)?,
        status: status_from(&r.get::<_, String>(5)?),
        issued_at: r.get(6)?,
        local_date: r.get(7)?,
        note: r.get(8)?,
        void_reason: r.get(9)?,
        print_count: r.get(10)?,
        created_by_name: r.get(11)?,
        base: r.get(12)?,
        version: r.get(13)?,
        amounts: Amounts::default(),
    })
    .and_then(|mut raw| {
        raw.amounts = conn.query_row(
            &format!("SELECT {AMOUNT_COLS} FROM invoice i WHERE i.id = ?1"),
            [&raw.id],
            |r| Amounts::from_row(r, 0),
        )?;
        Ok(raw)
    })
}

struct Raw {
    id: String,
    number: String,
    patient_id: String,
    patient_name: String,
    patient_number: String,
    status: InvoiceStatus,
    issued_at: String,
    local_date: String,
    note: Option<String>,
    void_reason: Option<String>,
    print_count: i64,
    created_by_name: Option<String>,
    base: i64,
    version: i64,
    amounts: Amounts,
}

fn finish(conn: &Connection, raw: Raw) -> Result<InvoiceInfo> {
    let a = raw.amounts;
    Ok(InvoiceInfo {
        lines: lines_of(conn, &raw.id)?,
        installments: installments_of(conn, &raw.id, raw.base, a.applied())?,
        credit_notes: credit_notes_of(conn, &raw.id)?,
        pay_state: pay_state(raw.status, &a),
        subtotal: a.subtotal,
        discount: a.discount,
        credit_notes_total: a.credit_notes,
        net: a.net(),
        paid: a.paid,
        refunded: a.refunded,
        remaining: if raw.status == InvoiceStatus::Void { 0 } else { a.remaining() },
        id: raw.id,
        number: raw.number,
        patient_id: raw.patient_id,
        patient_name: raw.patient_name,
        patient_number: raw.patient_number,
        status: raw.status,
        issued_at: raw.issued_at,
        local_date: raw.local_date,
        note: raw.note,
        void_reason: raw.void_reason,
        print_count: raw.print_count,
        created_by_name: raw.created_by_name,
        version: raw.version,
    })
}

pub fn get_invoice(conn: &Connection, id: &str) -> Result<InvoiceInfo> {
    let raw = conn
        .query_row(&format!("{INVOICE_SELECT} AND i.id = ?1"), [id], |r| hydrate(conn, r))
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("invoice {id}"))
                .on_field("invoice_id", ValidationRule::InvoiceNotFound)
        })?;
    finish(conn, raw)
}

pub fn list_invoices(conn: &Connection, p: &InvoiceListParams) -> Result<Vec<InvoiceInfo>> {
    let mut filter = String::new();
    let mut args: Vec<Value> = Vec::new();
    if let Some(pid) = p.patient_id.as_deref().filter(|s| !s.is_empty()) {
        filter.push_str(" AND i.patient_id = ?");
        args.push(Value::Text(pid.to_string()));
    }
    if let Some(from) = opt_date("from", &p.from)? {
        filter.push_str(" AND i.local_date >= ?");
        args.push(Value::Text(from));
    }
    if let Some(to) = opt_date("to", &p.to)? {
        filter.push_str(" AND i.local_date <= ?");
        args.push(Value::Text(to));
    }
    if let Some(q) = p.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let like = format!("%{}%", crate::normalize::normalize_for_search(q));
        filter.push_str(
            " AND (i.number LIKE ? OR p.patient_number LIKE ? OR p.full_name LIKE ? OR COALESCE(p.phone, '') LIKE ?)",
        );
        for _ in 0..4 {
            args.push(Value::Text(like.clone()));
        }
    }
    if p.open_only {
        filter.push_str(" AND i.status = 'issued'");
    }
    let limit = p.limit.unwrap_or(100).min(MAX_LIST) as usize;
    let offset = p.offset.unwrap_or(0) as usize;
    let sql = format!("{INVOICE_SELECT}{filter} ORDER BY i.issued_at DESC, i.number DESC");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(args.iter()))?;
    let mut out = Vec::new();
    let mut skipped = 0;
    while let Some(row) = rows.next()? {
        let raw = hydrate(conn, row)?;
        if p.open_only && raw.amounts.remaining() == 0 {
            continue;
        }
        if skipped < offset {
            skipped += 1;
            continue;
        }
        out.push(finish(conn, raw)?);
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

/// Completed treatments of the patient not on any invoice yet.
pub fn billable(conn: &Connection, patient_id: &str) -> Result<Vec<BillableItem>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.service_id, s.name_fa, s.name_ps, s.name_en, i.teeth, i.surfaces, i.units, i.unit_price,
            i.doctor_id, i.completed_at, i.details
         FROM treatment_item i JOIN service s ON s.id = i.service_id
         WHERE i.patient_id = ?1 AND i.status = 'completed' AND i.billed_at IS NULL AND i.deleted_at IS NULL
         ORDER BY COALESCE(i.completed_at, i.created_at), i.sort_order",
    )?;
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
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

// ───────────────────────────── invoices: writing ─────────────────────────────

fn service_names(conn: &Connection, service_id: &str) -> Result<(Translations, i64)> {
    conn.query_row(
        "SELECT name_fa, name_ps, name_en, price FROM service WHERE id = ?1 AND deleted_at IS NULL",
        [service_id],
        |r| Ok((trio(r.get(0)?, r.get(1)?, r.get(2)?), r.get::<_, i64>(3)?)),
    )
    .optional()?
    .ok_or_else(|| CoreError::invalid("service_id", ValidationRule::ServiceNotFound, "unknown service"))
}

struct NewLine {
    treatment_item_id: Option<String>,
    service_id: Option<String>,
    names: Translations,
    teeth: String,
    surfaces: String,
    doctor_id: Option<String>,
    quantity: i64,
    unit_price: i64,
    listed_price: i64,
}

fn check_doctor(conn: &Connection, doctor_id: &Option<String>) -> Result<Option<String>> {
    match doctor_id.as_deref().filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(d) => {
            crate::scheduling::get_doctor(conn, d)
                .map_err(|e| e.on_field("doctor_id", ValidationRule::DoctorNotFound))?;
            Ok(Some(d.to_string()))
        }
    }
}

pub fn create_invoice(
    conn: &Connection,
    actor: &Actor,
    can_override: bool,
    p: &CreateInvoiceParams,
) -> Result<InvoiceInfo> {
    check_patient(conn, &p.patient_id)?;
    if p.items.is_empty() && p.extra_lines.is_empty() {
        return Err(CoreError::invalid(
            "items",
            ValidationRule::InvoiceEmpty,
            "an invoice needs at least one line",
        ));
    }
    let mut lines: Vec<NewLine> = Vec::new();
    let mut overrides = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for pick in &p.items {
        if !seen.insert(pick.item_id.clone()) {
            continue;
        }
        let row = conn
            .query_row(
                "SELECT patient_id, service_id, teeth, surfaces, doctor_id, unit_price, units, status, billed_at
                 FROM treatment_item WHERE id = ?1 AND deleted_at IS NULL",
                [&pick.item_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, i64>(6)?,
                        r.get::<_, String>(7)?,
                        r.get::<_, Option<String>>(8)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::invalid("items", ValidationRule::ItemNotBillable, "unknown treatment"))?;
        let (patient, service, teeth, surfaces, doctor, price, units, status, billed) = row;
        if patient != p.patient_id || status != "completed" {
            return Err(CoreError::invalid(
                "items",
                ValidationRule::ItemNotBillable,
                "this treatment cannot be invoiced",
            ));
        }
        if billed.is_some() {
            return Err(CoreError::invalid(
                "items",
                ValidationRule::ItemAlreadyBilled,
                "this treatment is already invoiced",
            ));
        }
        let (names, _) = service_names(conn, &service)?;
        let unit_price = money("unit_price", pick.unit_price.unwrap_or(price), true)?;
        if unit_price != price {
            if !can_override {
                return Err(CoreError::forbidden(crate::auth::perm::BILLING_OVERRIDE));
            }
            overrides.push(json!({"item_id": pick.item_id, "from": price, "to": unit_price}));
        }
        lines.push(NewLine {
            treatment_item_id: Some(pick.item_id.clone()),
            service_id: Some(service),
            names,
            teeth,
            surfaces,
            doctor_id: doctor,
            quantity: units,
            unit_price,
            listed_price: price,
        });
    }
    for x in &p.extra_lines {
        if !(1..=1000).contains(&x.quantity) {
            return Err(CoreError::invalid(
                "quantity",
                ValidationRule::BillingAmount,
                "quantity is 1 to 1000",
            ));
        }
        let unit_price = money("unit_price", x.unit_price, true)?;
        let doctor_id = check_doctor(conn, &x.doctor_id)?;
        let (names, listed, service_id) = match x.service_id.as_deref().filter(|s| !s.is_empty()) {
            Some(sid) => {
                let (names, price) = service_names(conn, sid)?;
                if unit_price != price {
                    if !can_override {
                        return Err(CoreError::forbidden(crate::auth::perm::BILLING_OVERRIDE));
                    }
                    overrides.push(json!({"service_id": sid, "from": price, "to": unit_price}));
                }
                (names, price, Some(sid.to_string()))
            }
            None => {
                let d = required("description", x.description.as_deref().unwrap_or(""), 120)?;
                (trio(d.clone(), d.clone(), d), 0, None)
            }
        };
        lines.push(NewLine {
            treatment_item_id: None,
            service_id,
            names,
            teeth: x.teeth.iter().map(u8::to_string).collect::<Vec<_>>().join(","),
            surfaces: String::new(),
            doctor_id,
            quantity: x.quantity,
            unit_price,
            listed_price: listed,
        });
    }
    let subtotal: i64 = lines.iter().map(|l| l.unit_price * l.quantity).sum();
    if subtotal > MAX_AMOUNT {
        return Err(CoreError::invalid(
            "items",
            ValidationRule::BillingAmount,
            "the invoice total is too large",
        ));
    }
    let discount = money("discount", p.discount, true)?;
    if discount > subtotal {
        return Err(CoreError::invalid(
            "discount",
            ValidationRule::DiscountRange,
            "the discount is more than the total",
        ));
    }
    let note = optional("note", &p.note, 500)?;
    let id = new_id();
    let number = next_number(conn, "INV")?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO invoice(id, number, patient_id, issued_at, local_date, discount, note, created_at, created_by, updated_at, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?4, ?8, ?4, ?8)",
        params![id, number, p.patient_id, now, today_iso(), discount, note, actor.user_id],
    )?;
    for (i, l) in lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO invoice_line(id, invoice_id, treatment_item_id, service_id, names_json, teeth, surfaces, doctor_id,
                quantity, unit_price, listed_price, line_total, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                new_id(),
                id,
                l.treatment_item_id,
                l.service_id,
                serde_json::to_string(&l.names)?,
                l.teeth,
                l.surfaces,
                l.doctor_id,
                l.quantity,
                l.unit_price,
                l.listed_price,
                l.unit_price * l.quantity,
                i as i64
            ],
        )?;
        if let Some(item) = &l.treatment_item_id {
            conn.execute("UPDATE treatment_item SET billed_at = ?1 WHERE id = ?2", params![now, item])?;
        }
    }
    if !p.installments.is_empty() {
        write_installments(conn, &id, &p.installments)?;
    }
    let after = get_invoice(conn, &id)?;
    audit::record(
        conn,
        actor,
        "invoice.create",
        Some("patient"),
        Some(&p.patient_id),
        None,
        Some(
            &json!({"id": id, "number": number, "net": after.net, "discount": discount, "price_overrides": overrides}),
        ),
    )?;
    Ok(after)
}

fn issued(conn: &Connection, id: &str) -> Result<InvoiceInfo> {
    let inv = get_invoice(conn, id)?;
    if inv.status == InvoiceStatus::Void {
        return Err(CoreError::invalid("invoice_id", ValidationRule::InvoiceVoid, "this invoice was voided"));
    }
    Ok(inv)
}

pub fn void_invoice(conn: &Connection, actor: &Actor, p: &VoidInvoiceParams) -> Result<InvoiceInfo> {
    let before = issued(conn, &p.id)?;
    let reason = required("reason", &p.reason, 300)?;
    if before.paid - before.refunded > 0 {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::InvoiceHasPayments,
            "refund the payments of this invoice before voiding it",
        ));
    }
    let now = now_iso();
    let changed = conn.execute(
        "UPDATE invoice SET status = 'void', void_reason = ?1, voided_at = ?2, updated_at = ?2, updated_by = ?3, version = version + 1
         WHERE id = ?4 AND version = ?5 AND deleted_at IS NULL",
        params![reason, now, actor.user_id, p.id, p.version],
    )?;
    expect_one_row(changed, "invoice")?;
    // The treatments go back to "ready for an invoice".
    conn.execute(
        "UPDATE treatment_item SET billed_at = NULL
         WHERE id IN (SELECT treatment_item_id FROM invoice_line WHERE invoice_id = ?1 AND treatment_item_id IS NOT NULL)",
        [&p.id],
    )?;
    let after = get_invoice(conn, &p.id)?;
    audit::record(
        conn,
        actor,
        "invoice.void",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!({"id": before.id, "number": before.number, "net": before.net})),
        Some(&json!({"reason": reason})),
    )?;
    Ok(after)
}

pub fn credit_note(conn: &Connection, actor: &Actor, p: &CreditNoteParams) -> Result<InvoiceInfo> {
    let before = issued(conn, &p.invoice_id)?;
    let amount = money("amount", p.amount, false)?;
    let reason = required("reason", &p.reason, 300)?;
    if amount > before.remaining {
        return Err(CoreError::invalid(
            "amount",
            ValidationRule::CreditNoteExceeds,
            "a credit note cannot be more than what is still owed on the invoice",
        ));
    }
    let number = next_number(conn, "CN")?;
    conn.execute(
        "INSERT INTO credit_note(id, number, invoice_id, patient_id, amount, reason, issued_at, local_date, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![new_id(), number, p.invoice_id, before.patient_id, amount, reason, now_iso(), today_iso(), actor.user_id],
    )?;
    conn.execute(
        "UPDATE invoice SET updated_at = ?1, updated_by = ?2, version = version + 1 WHERE id = ?3",
        params![now_iso(), actor.user_id, p.invoice_id],
    )?;
    let after = get_invoice(conn, &p.invoice_id)?;
    audit::record(
        conn,
        actor,
        "invoice.credit_note",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!({"id": before.id, "number": before.number, "net": before.net})),
        Some(&json!({"credit_note": number, "amount": amount, "reason": reason})),
    )?;
    Ok(after)
}

fn write_installments(conn: &Connection, invoice_id: &str, list: &[NewInstallment]) -> Result<()> {
    let a = amounts(conn, invoice_id)?;
    let mut rows = Vec::new();
    for x in list {
        rows.push((date_of("due_date", &x.due_date)?, money("amount", x.amount, false)?));
    }
    rows.sort();
    let sum: i64 = rows.iter().map(|r| r.1).sum();
    if rows.len() > 60 || (!rows.is_empty() && sum != a.remaining()) {
        return Err(CoreError::invalid(
            "installments",
            ValidationRule::InstallmentsInvalid,
            "the installments must add up to what is still owed",
        ));
    }
    conn.execute("DELETE FROM installment WHERE invoice_id = ?1", [invoice_id])?;
    for (i, (due, amount)) in rows.iter().enumerate() {
        conn.execute(
            "INSERT INTO installment(id, invoice_id, no, due_date, amount) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![new_id(), invoice_id, i as i64 + 1, due, amount],
        )?;
    }
    conn.execute("UPDATE invoice SET installment_base = ?1 WHERE id = ?2", params![a.applied(), invoice_id])?;
    Ok(())
}

pub fn set_installments(conn: &Connection, actor: &Actor, p: &SetInstallmentsParams) -> Result<InvoiceInfo> {
    let before = issued(conn, &p.invoice_id)?;
    write_installments(conn, &p.invoice_id, &p.installments)?;
    let after = get_invoice(conn, &p.invoice_id)?;
    audit::record(
        conn,
        actor,
        "invoice.installments",
        Some("patient"),
        Some(&before.patient_id),
        Some(&json!({"id": before.id, "number": before.number, "count": before.installments.len()})),
        Some(&json!({"count": after.installments.len(), "remaining": after.remaining})),
    )?;
    Ok(after)
}

pub fn invoice_printed(conn: &Connection, actor: &Actor, id: &str, pdf: bool) -> Result<InvoiceInfo> {
    let inv = issued(conn, id)?;
    conn.execute(
        "UPDATE invoice SET print_count = print_count + 1, last_printed_at = ?1 WHERE id = ?2",
        params![now_iso(), id],
    )?;
    audit::record(
        conn,
        actor,
        if pdf { "invoice.pdf" } else { "invoice.print" },
        Some("patient"),
        Some(&inv.patient_id),
        None,
        Some(&json!({"id": inv.id, "number": inv.number, "copy": inv.print_count + 1})),
    )?;
    get_invoice(conn, id)
}

// ───────────────────────────── payments ─────────────────────────────

const PAYMENT_SELECT: &str = "SELECT p.id, p.number, p.kind, p.patient_id, pt.full_name, p.invoice_id, i.number, p.method_id,
    m.name_fa, m.name_ps, m.name_en, p.amount, p.paid_at, p.local_date, p.note, p.refund_of, p.returns_to_credit, p.reason,
    (SELECT COALESCE(SUM(r.amount), 0) FROM payment r WHERE r.refund_of = p.id),
    p.print_count, p.last_printed_at, p.receipt_json, u.display_name, p.version
    FROM payment p JOIN patient pt ON pt.id = p.patient_id
    LEFT JOIN invoice i ON i.id = p.invoice_id
    LEFT JOIN payment_method m ON m.id = p.method_id
    LEFT JOIN app_user u ON u.id = p.created_by
    WHERE 1 = 1";

fn kind_code(k: PaymentKind) -> &'static str {
    match k {
        PaymentKind::Payment => "payment",
        PaymentKind::Credit => "credit",
        PaymentKind::Refund => "refund",
    }
}

fn kind_from(s: &str) -> PaymentKind {
    match s {
        "credit" => PaymentKind::Credit,
        "refund" => PaymentKind::Refund,
        _ => PaymentKind::Payment,
    }
}

fn map_payment(r: &rusqlite::Row<'_>) -> rusqlite::Result<(PaymentInfo, String)> {
    let method = match r.get::<_, Option<String>>(8)? {
        Some(fa) => Some(trio(fa, r.get(9)?, r.get(10)?)),
        None => None,
    };
    Ok((
        PaymentInfo {
            id: r.get(0)?,
            number: r.get(1)?,
            kind: kind_from(&r.get::<_, String>(2)?),
            patient_id: r.get(3)?,
            patient_name: r.get(4)?,
            invoice_id: r.get(5)?,
            invoice_number: r.get(6)?,
            method_id: r.get(7)?,
            method,
            amount: r.get(11)?,
            paid_at: r.get(12)?,
            local_date: r.get(13)?,
            note: r.get(14)?,
            refund_of: r.get(15)?,
            returns_to_credit: r.get::<_, i64>(16)? == 1,
            reason: r.get(17)?,
            refunded: r.get(18)?,
            print_count: r.get(19)?,
            last_printed_at: r.get(20)?,
            receipt: ReceiptSnapshot {
                patient_name: String::new(),
                patient_number: String::new(),
                invoice_number: None,
                lines: Vec::new(),
                invoice_net: 0,
                paid_after: 0,
                remaining_after: 0,
                credit_after: 0,
                method: None,
            },
            created_by_name: r.get(22)?,
            version: r.get(23)?,
        },
        r.get::<_, String>(21)?,
    ))
}

fn with_receipt((mut p, json): (PaymentInfo, String)) -> Result<PaymentInfo> {
    p.receipt = serde_json::from_str(&json)?;
    Ok(p)
}

pub fn get_payment(conn: &Connection, id: &str) -> Result<PaymentInfo> {
    let row = conn
        .query_row(&format!("{PAYMENT_SELECT} AND p.id = ?1"), [id], map_payment)
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("payment {id}"))
                .on_field("payment_id", ValidationRule::PaymentNotFound)
        })?;
    with_receipt(row)
}

pub fn list_payments(conn: &Connection, p: &PaymentListParams) -> Result<Vec<PaymentInfo>> {
    let mut filter = String::new();
    let mut args: Vec<Value> = Vec::new();
    if let Some(v) = p.patient_id.as_deref().filter(|s| !s.is_empty()) {
        filter.push_str(" AND p.patient_id = ?");
        args.push(Value::Text(v.to_string()));
    }
    if let Some(v) = p.invoice_id.as_deref().filter(|s| !s.is_empty()) {
        filter.push_str(" AND p.invoice_id = ?");
        args.push(Value::Text(v.to_string()));
    }
    if let Some(v) = opt_date("from", &p.from)? {
        filter.push_str(" AND p.local_date >= ?");
        args.push(Value::Text(v));
    }
    if let Some(v) = opt_date("to", &p.to)? {
        filter.push_str(" AND p.local_date <= ?");
        args.push(Value::Text(v));
    }
    args.push(Value::Integer(p.limit.unwrap_or(200).min(MAX_LIST) as i64));
    args.push(Value::Integer(p.offset.unwrap_or(0) as i64));
    let sql = format!("{PAYMENT_SELECT}{filter} ORDER BY p.paid_at DESC, p.number DESC LIMIT ? OFFSET ?");
    let mut stmt = conn.prepare(&sql)?;
    let rows: Vec<(PaymentInfo, String)> =
        stmt.query_map(params_from_iter(args.iter()), map_payment)?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(with_receipt).collect()
}

struct NewPayment<'a> {
    kind: PaymentKind,
    patient_id: &'a str,
    invoice_id: Option<&'a str>,
    method_id: Option<&'a str>,
    amount: i64,
    note: Option<String>,
    refund_of: Option<&'a str>,
    returns_to_credit: bool,
    reason: Option<String>,
}

fn snapshot(
    conn: &Connection,
    patient_id: &str,
    invoice_id: Option<&str>,
    method_id: Option<&str>,
) -> Result<ReceiptSnapshot> {
    let (patient_name, patient_number): (String, String) =
        conn.query_row("SELECT full_name, patient_number FROM patient WHERE id = ?1", [patient_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    let method = match method_id {
        Some(m) => Some(get_method(conn, m)?.name),
        None => None,
    };
    let mut snap = ReceiptSnapshot {
        patient_name,
        patient_number,
        invoice_number: None,
        lines: Vec::new(),
        invoice_net: 0,
        paid_after: 0,
        remaining_after: 0,
        credit_after: patient_credit(conn, patient_id)?,
        method,
    };
    if let Some(inv) = invoice_id {
        let a = amounts(conn, inv)?;
        snap.invoice_number =
            Some(conn.query_row("SELECT number FROM invoice WHERE id = ?1", [inv], |r| r.get(0))?);
        snap.lines = lines_of(conn, inv)?
            .into_iter()
            .take(RECEIPT_LINES)
            .map(|l| ReceiptLine { names: l.names, amount: l.line_total })
            .collect();
        snap.invoice_net = a.net();
        snap.paid_after = a.applied();
        snap.remaining_after = a.remaining();
    }
    Ok(snap)
}

fn insert_payment(conn: &Connection, actor: &Actor, n: NewPayment<'_>) -> Result<PaymentInfo> {
    let prefix = match n.kind {
        PaymentKind::Payment => "RC",
        PaymentKind::Credit => "CU",
        PaymentKind::Refund => "RV",
    };
    let id = new_id();
    let number = next_number(conn, prefix)?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO payment(id, number, kind, patient_id, invoice_id, method_id, amount, paid_at, local_date, note, refund_of,
            returns_to_credit, reason, receipt_json, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, '{}', ?8, ?14)",
        params![
            id,
            number,
            kind_code(n.kind),
            n.patient_id,
            n.invoice_id,
            n.method_id,
            n.amount,
            now,
            today_iso(),
            n.note,
            n.refund_of,
            n.returns_to_credit,
            n.reason,
            actor.user_id
        ],
    )?;
    // The receipt says what is true after this row, so it is read back from the database.
    let snap = snapshot(conn, n.patient_id, n.invoice_id, n.method_id)?;
    conn.execute(
        "UPDATE payment SET receipt_json = ?1 WHERE id = ?2",
        params![serde_json::to_string(&snap)?, id],
    )?;
    let after = get_payment(conn, &id)?;
    audit::record(
        conn,
        actor,
        match n.kind {
            PaymentKind::Payment => "payment.add",
            PaymentKind::Credit => "payment.credit",
            PaymentKind::Refund => "payment.refund",
        },
        Some("patient"),
        Some(n.patient_id),
        None,
        Some(&json!({
            "id": id, "number": number, "amount": n.amount, "invoice": after.invoice_number,
            "refund_of": n.refund_of, "reason": n.reason
        })),
    )?;
    Ok(after)
}

pub fn add_payment(conn: &Connection, actor: &Actor, p: &AddPaymentParams) -> Result<PaymentInfo> {
    check_patient(conn, &p.patient_id)?;
    let amount = money("amount", p.amount, false)?;
    let note = optional("note", &p.note, 300)?;
    let invoice_id = p.invoice_id.as_deref().filter(|s| !s.is_empty());
    if let Some(inv_id) = invoice_id {
        let inv = issued(conn, inv_id)?;
        if inv.patient_id != p.patient_id {
            return Err(CoreError::invalid(
                "invoice_id",
                ValidationRule::InvalidParams,
                "that invoice is another patient's",
            ));
        }
        if amount > inv.remaining {
            return Err(CoreError::invalid(
                "amount",
                ValidationRule::PaymentExceeds,
                "the payment is more than what is still owed; record the rest as an advance",
            ));
        }
    }
    if p.use_credit {
        if invoice_id.is_none() {
            return Err(CoreError::invalid(
                "invoice_id",
                ValidationRule::Required,
                "credit is used on an invoice",
            ));
        }
        if amount > patient_credit(conn, &p.patient_id)? {
            return Err(CoreError::invalid(
                "amount",
                ValidationRule::InsufficientCredit,
                "the patient has less credit than that",
            ));
        }
        return insert_payment(
            conn,
            actor,
            NewPayment {
                kind: PaymentKind::Credit,
                patient_id: &p.patient_id,
                invoice_id,
                method_id: None,
                amount,
                note,
                refund_of: None,
                returns_to_credit: false,
                reason: None,
            },
        );
    }
    let method_id = p.method_id.as_deref().filter(|s| !s.is_empty()).ok_or_else(|| {
        CoreError::invalid("method_id", ValidationRule::PaymentMethodNotFound, "choose how it was paid")
    })?;
    let method = get_method(conn, method_id)?;
    if !method.is_active {
        return Err(CoreError::invalid(
            "method_id",
            ValidationRule::PaymentMethodNotFound,
            "that payment method is turned off",
        ));
    }
    insert_payment(
        conn,
        actor,
        NewPayment {
            kind: PaymentKind::Payment,
            patient_id: &p.patient_id,
            invoice_id,
            method_id: Some(method_id),
            amount,
            note,
            refund_of: None,
            returns_to_credit: false,
            reason: None,
        },
    )
}

pub fn refund(conn: &Connection, actor: &Actor, p: &RefundParams) -> Result<PaymentInfo> {
    let original = get_payment(conn, &p.payment_id)?;
    if original.kind == PaymentKind::Refund {
        return Err(CoreError::invalid(
            "payment_id",
            ValidationRule::InvalidParams,
            "a refund cannot be refunded",
        ));
    }
    let amount = money("amount", p.amount, false)?;
    let reason = required("reason", &p.reason, 300)?;
    if amount > original.amount - original.refunded {
        return Err(CoreError::invalid(
            "amount",
            ValidationRule::RefundExceeds,
            "more than what is left of that payment",
        ));
    }
    let advance = original.kind == PaymentKind::Payment && original.invoice_id.is_none();
    if advance && amount > patient_credit(conn, &original.patient_id)? {
        return Err(CoreError::invalid(
            "amount",
            ValidationRule::InsufficientCredit,
            "that credit was already used",
        ));
    }
    let to_credit = original.kind == PaymentKind::Credit || (p.to_credit && !advance);
    insert_payment(
        conn,
        actor,
        NewPayment {
            kind: PaymentKind::Refund,
            patient_id: &original.patient_id,
            invoice_id: original.invoice_id.as_deref(),
            method_id: original.method_id.as_deref(),
            amount,
            note: None,
            refund_of: Some(&original.id),
            returns_to_credit: to_credit,
            reason: Some(reason),
        },
    )
}

pub fn payment_printed(conn: &Connection, actor: &Actor, id: &str, pdf: bool) -> Result<PaymentInfo> {
    let p = get_payment(conn, id)?;
    conn.execute(
        "UPDATE payment SET print_count = print_count + 1, last_printed_at = ?1 WHERE id = ?2",
        params![now_iso(), id],
    )?;
    audit::record(
        conn,
        actor,
        if pdf { "payment.pdf" } else { "payment.print" },
        Some("patient"),
        Some(&p.patient_id),
        None,
        Some(&json!({"id": p.id, "number": p.number, "copy": p.print_count + 1})),
    )?;
    get_payment(conn, id)
}

// ───────────────────────────── balance, debtors, statement ─────────────────────────────

pub fn account(conn: &Connection, patient_id: &str) -> Result<PatientAccount> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {AMOUNT_COLS} FROM invoice i WHERE i.patient_id = ?1 AND i.status = 'issued' AND i.deleted_at IS NULL"
    ))?;
    let all: Vec<Amounts> =
        stmt.query_map([patient_id], |r| Amounts::from_row(r, 0))?.collect::<rusqlite::Result<_>>()?;
    let (unbilled_count, unbilled_total) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(unit_price * units), 0) FROM treatment_item
         WHERE patient_id = ?1 AND status = 'completed' AND billed_at IS NULL AND deleted_at IS NULL",
        [patient_id],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )?;
    Ok(PatientAccount {
        patient_id: patient_id.to_string(),
        invoice_count: all.len() as i64,
        total: all.iter().map(|a| a.subtotal).sum(),
        discount: all.iter().map(|a| a.discount + a.credit_notes).sum(),
        paid: all.iter().map(|a| a.paid).sum(),
        refunded: all.iter().map(|a| a.refunded).sum(),
        remaining: all.iter().map(Amounts::remaining).sum(),
        credit: patient_credit(conn, patient_id)?,
        unbilled_count,
        unbilled_total,
    })
}

/// `(invoice id, patient id, name, number, phone, issued at, installment base, amounts)`.
type DebtorRow = (String, String, String, String, Option<String>, String, i64, Amounts);

pub fn debtors(conn: &Connection, p: &DebtorListParams) -> Result<Vec<DebtorInfo>> {
    let mut sql = format!(
        "SELECT i.id, i.patient_id, p.full_name, p.patient_number, p.phone, i.issued_at, i.installment_base, {AMOUNT_COLS}
         FROM invoice i JOIN patient p ON p.id = i.patient_id
         WHERE i.status = 'issued' AND i.deleted_at IS NULL AND p.deleted_at IS NULL"
    );
    let mut args: Vec<Value> = Vec::new();
    if let Some(q) = p.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let like = format!("%{}%", crate::normalize::normalize_for_search(q));
        sql.push_str(" AND (p.full_name LIKE ? OR p.patient_number LIKE ? OR COALESCE(p.phone, '') LIKE ?)");
        for _ in 0..3 {
            args.push(Value::Text(like.clone()));
        }
    }
    let mut stmt = conn.prepare(&sql)?;
    let rows: Vec<DebtorRow> = stmt
        .query_map(params_from_iter(args.iter()), |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                Amounts::from_row(r, 7)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut by_patient: BTreeMap<String, DebtorInfo> = BTreeMap::new();
    for (invoice_id, patient_id, name, number, phone, issued_at, base, a) in rows {
        if a.remaining() == 0 {
            continue;
        }
        let d = by_patient.entry(patient_id.clone()).or_insert_with(|| DebtorInfo {
            patient_id: patient_id.clone(),
            patient_name: name,
            patient_number: number,
            phone,
            remaining: 0,
            invoice_count: 0,
            oldest_invoice_at: None,
            last_payment_at: None,
            next_due: None,
            overdue: false,
        });
        d.remaining += a.remaining();
        d.invoice_count += 1;
        if d.oldest_invoice_at.as_ref().is_none_or(|o| issued_at < *o) {
            d.oldest_invoice_at = Some(issued_at);
        }
        for i in installments_of(conn, &invoice_id, base, a.applied())? {
            if i.state == InstallmentState::Paid {
                continue;
            }
            if i.state == InstallmentState::Overdue {
                d.overdue = true;
            }
            if d.next_due.as_ref().is_none_or(|n| i.due_date < *n) {
                d.next_due = Some(i.due_date);
            }
        }
    }
    let mut out: Vec<DebtorInfo> = by_patient.into_values().collect();
    for d in &mut out {
        d.last_payment_at = conn
            .query_row(
                "SELECT MAX(paid_at) FROM payment WHERE patient_id = ?1 AND kind = 'payment'",
                [&d.patient_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
    }
    out.sort_by(|a, b| b.remaining.cmp(&a.remaining).then_with(|| a.patient_name.cmp(&b.patient_name)));
    out.truncate(p.limit.unwrap_or(200).min(1000) as usize);
    Ok(out)
}

/// One line of the statement before its running balance: `(local date, sort key, line)`.
type Entry = (String, String, StatementLine);

pub fn statement(conn: &Connection, p: &StatementParams) -> Result<StatementInfo> {
    let patient = crate::patient::get_patient(conn, &p.patient_id)?;
    let from = opt_date("from", &p.from)?;
    let to = opt_date("to", &p.to)?;
    if let (Some(f), Some(t)) = (&from, &to) {
        if f > t {
            return Err(CoreError::invalid("to", ValidationRule::PeriodRange, "the end is before the start"));
        }
    }
    let mut entries: Vec<Entry> = Vec::new();
    let line =
        |at: &str, kind: &str, number: &str, description: String, debit: i64, credit: i64| StatementLine {
            at: at.to_string(),
            kind: kind.to_string(),
            number: number.to_string(),
            description,
            debit,
            credit,
            balance: 0,
        };
    // Invoices (not void): what they charge is the total less the discount; credit notes lower it.
    let mut stmt = conn.prepare(&format!(
        "SELECT i.number, i.issued_at, i.local_date, {AMOUNT_COLS} FROM invoice i
         WHERE i.patient_id = ?1 AND i.status = 'issued' AND i.deleted_at IS NULL"
    ))?;
    let invoices: Vec<(String, String, String, Amounts)> = stmt
        .query_map([&p.patient_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, Amounts::from_row(r, 3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (number, at, date, a) in invoices {
        entries.push((
            date,
            format!("{at}1"),
            line(&at, "invoice", &number, String::new(), a.subtotal - a.discount, 0),
        ));
    }
    let mut stmt = conn.prepare(
        "SELECT c.number, c.issued_at, c.local_date, c.amount, c.reason, i.number FROM credit_note c
         JOIN invoice i ON i.id = c.invoice_id WHERE c.patient_id = ?1 AND i.status = 'issued'",
    )?;
    let notes: Vec<(String, String, String, i64, String, String)> = stmt
        .query_map([&p.patient_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    for (number, at, date, amount, reason, inv) in notes {
        entries.push((
            date,
            format!("{at}2"),
            line(&at, "credit_note", &number, format!("{inv} — {reason}"), 0, amount),
        ));
    }
    for pay in payments_of_patient(conn, &p.patient_id)? {
        // Credit put on an invoice and refunds into credit move money between the patient's own
        // balances: they change nothing in what the patient owes.
        let desc = pay.invoice_number.clone().unwrap_or_default();
        let key = format!("{}3", pay.paid_at);
        match pay.kind {
            PaymentKind::Payment => entries.push((
                pay.local_date.clone(),
                key,
                line(&pay.paid_at, "payment", &pay.number, desc, 0, pay.amount),
            )),
            PaymentKind::Refund if !pay.returns_to_credit => entries.push((
                pay.local_date.clone(),
                key,
                line(&pay.paid_at, "refund", &pay.number, desc, pay.amount, 0),
            )),
            _ => {}
        }
    }
    entries.sort_by(|a, b| a.1.cmp(&b.1));
    let mut opening = 0;
    let mut running = 0;
    let mut lines = Vec::new();
    for (date, _, mut l) in entries {
        running += l.debit - l.credit;
        if from.as_ref().is_some_and(|f| date < *f) {
            opening = running;
            continue;
        }
        if to.as_ref().is_some_and(|t| date > *t) {
            continue;
        }
        l.balance = running;
        lines.push(l);
    }
    Ok(StatementInfo {
        patient_id: patient.id.clone(),
        patient_name: patient.full_name,
        patient_number: patient.patient_number,
        phone: patient.phone,
        from,
        to,
        generated_at: now_iso(),
        opening_balance: opening,
        lines,
        account: account(conn, &p.patient_id)?,
    })
}

/// Every money row of the patient, oldest first (the statement needs them all).
fn payments_of_patient(conn: &Connection, patient_id: &str) -> Result<Vec<PaymentInfo>> {
    let mut stmt =
        conn.prepare(&format!("{PAYMENT_SELECT} AND p.patient_id = ?1 ORDER BY p.paid_at, p.number"))?;
    let rows: Vec<(PaymentInfo, String)> =
        stmt.query_map([patient_id], map_payment)?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(with_receipt).collect()
}

pub fn installments_due(conn: &Connection, p: &InstallmentsDueParams) -> Result<Vec<InstallmentDue>> {
    let through = match opt_date("through", &p.through)? {
        Some(t) => t,
        None => format_date(local_now().date() + time::Duration::days(7)),
    };
    let mut stmt = conn.prepare(
        "SELECT DISTINCT i.id FROM invoice i JOIN installment n ON n.invoice_id = i.id
         WHERE i.status = 'issued' AND i.deleted_at IS NULL AND n.due_date <= ?1",
    )?;
    let ids: Vec<String> = stmt.query_map([&through], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for id in ids {
        let inv = get_invoice(conn, &id)?;
        let phone: Option<String> =
            conn.query_row("SELECT phone FROM patient WHERE id = ?1", [&inv.patient_id], |r| r.get(0))?;
        for i in &inv.installments {
            if i.state == InstallmentState::Paid || i.due_date > through {
                continue;
            }
            out.push(InstallmentDue {
                invoice_id: inv.id.clone(),
                invoice_number: inv.number.clone(),
                patient_id: inv.patient_id.clone(),
                patient_name: inv.patient_name.clone(),
                phone: phone.clone(),
                no: i.no,
                due_date: i.due_date.clone(),
                amount: i.amount,
                unpaid: i.amount - i.paid,
                overdue: i.state == InstallmentState::Overdue,
            });
        }
    }
    out.sort_by(|a, b| a.due_date.cmp(&b.due_date).then_with(|| a.patient_name.cmp(&b.patient_name)));
    Ok(out)
}

// ───────────────────────────── settings & commission ─────────────────────────────

pub fn get_settings(conn: &Connection) -> Result<BillingSettings> {
    Ok(BillingSettings {
        commission_enabled: setting(conn, "billing.commission_enabled")?.as_deref() == Some("1"),
        commission_basis: match setting(conn, "billing.commission_basis")?.as_deref() {
            Some("treatment") => CommissionBasis::Treatment,
            _ => CommissionBasis::Payment,
        },
        receipt_footer: setting(conn, "billing.receipt_footer")?.unwrap_or_default(),
    })
}

pub fn save_settings(conn: &Connection, actor: &Actor, p: &BillingSettings) -> Result<BillingSettings> {
    let before = get_settings(conn)?;
    let footer = p.receipt_footer.trim();
    if footer.chars().count() > 200 {
        return Err(CoreError::invalid(
            "receipt_footer",
            ValidationRule::DocumentText,
            "the footer is at most 200 characters",
        ));
    }
    set_setting(conn, actor, "billing.commission_enabled", if p.commission_enabled { "1" } else { "0" })?;
    set_setting(
        conn,
        actor,
        "billing.commission_basis",
        match p.commission_basis {
            CommissionBasis::Payment => "payment",
            CommissionBasis::Treatment => "treatment",
        },
    )?;
    set_setting(conn, actor, "billing.receipt_footer", footer)?;
    let after = get_settings(conn)?;
    audit::record(
        conn,
        actor,
        "billing.settings",
        Some("setting"),
        None,
        Some(
            &json!({"commission_enabled": before.commission_enabled, "basis": before.commission_basis, "footer": before.receipt_footer}),
        ),
        Some(
            &json!({"commission_enabled": after.commission_enabled, "basis": after.commission_basis, "footer": after.receipt_footer}),
        ),
    )?;
    Ok(after)
}

fn commission_kind_from(s: &str) -> CommissionKind {
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
                kind: commission_kind_from(&r.get::<_, String>(3)?),
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
            service_names(conn, s)?;
            Some(s.to_string())
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
            "the commission value is out of range",
        ));
    }
    let kind = if p.kind == CommissionKind::Fixed { "fixed" } else { "percent" };
    let now = now_iso();
    // One rule per doctor/service pair: saving the same pair again changes that rule.
    let existing: Option<(String, i64)> = conn
        .query_row(
            "SELECT id, version FROM commission_rule
             WHERE COALESCE(doctor_id, '') = COALESCE(?1, '') AND COALESCE(service_id, '') = COALESCE(?2, '') AND deleted_at IS NULL",
            params![doctor, service],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let id = match (p.id.as_deref().filter(|s| !s.is_empty()), existing) {
        (Some(id), _) => {
            let version = p.version.unwrap_or(1);
            let changed = conn.execute(
                "UPDATE commission_rule SET doctor_id = ?1, service_id = ?2, kind = ?3, value = ?4, updated_at = ?5, updated_by = ?6,
                    version = version + 1 WHERE id = ?7 AND version = ?8 AND deleted_at IS NULL",
                params![doctor, service, kind, p.value, now, actor.user_id, id, version],
            )?;
            expect_one_row(changed, "commission rule")?;
            id.to_string()
        }
        (None, Some((id, _))) => {
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

fn commission_of(rule: Option<&CommissionRuleInfo>, base: i64, quantity: i64) -> i64 {
    match rule {
        None => 0,
        Some(r) => match r.kind {
            CommissionKind::Percent => ((base as i128) * (r.value as i128) / 10_000) as i64,
            CommissionKind::Fixed => r.value * quantity,
        },
    }
}

struct CommissionLineRow {
    doctor_id: String,
    service_id: Option<String>,
    names: Translations,
    quantity: i64,
    line_total: i64,
}

/// What a line really earns: its share of the invoice net (discount and credit notes taken off).
fn line_net(line_total: i64, a: &Amounts) -> i64 {
    if a.subtotal <= 0 {
        return 0;
    }
    ((line_total as i128) * (a.net() as i128) / (a.subtotal as i128)) as i64
}

pub fn commission_report(conn: &Connection, p: &CommissionReportParams) -> Result<CommissionReport> {
    let from = date_of("from", &p.from)?;
    let to = date_of("to", &p.to)?;
    if from > to {
        return Err(CoreError::invalid("to", ValidationRule::PeriodRange, "the end is before the start"));
    }
    let settings = get_settings(conn)?;
    let mut report = CommissionReport {
        basis: settings.commission_basis,
        from: from.clone(),
        to: to.clone(),
        doctors: Vec::new(),
        base_total: 0,
        commission_total: 0,
    };
    if !settings.commission_enabled {
        return Ok(report);
    }
    let rules = list_rules(conn)?;
    let mut doctor_names: HashMap<String, String> = HashMap::new();
    let mut by_doctor: BTreeMap<String, Vec<CommissionLine>> = BTreeMap::new();
    let mut push = |doctor: &str, line: CommissionLine, names: &mut HashMap<String, String>| -> Result<()> {
        if !names.contains_key(doctor) {
            names.insert(doctor.to_string(), crate::scheduling::get_doctor(conn, doctor)?.full_name);
        }
        by_doctor.entry(doctor.to_string()).or_default().push(line);
        Ok(())
    };
    let doctor_filter = p.doctor_id.as_deref().filter(|s| !s.is_empty());
    let invoice_lines = |invoice_id: &str| -> Result<Vec<CommissionLineRow>> {
        let mut stmt = conn.prepare_cached(
            "SELECT doctor_id, service_id, names_json, quantity, line_total FROM invoice_line
             WHERE invoice_id = ?1 AND doctor_id IS NOT NULL ORDER BY sort_order",
        )?;
        let rows: Vec<(String, Option<String>, String, i64, i64)> = stmt
            .query_map([invoice_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        rows.into_iter()
            .map(|(doctor_id, service_id, names, quantity, line_total)| {
                Ok(CommissionLineRow {
                    doctor_id,
                    service_id,
                    names: serde_json::from_str(&names)?,
                    quantity,
                    line_total,
                })
            })
            .collect()
    };
    match settings.commission_basis {
        CommissionBasis::Treatment => {
            let mut stmt = conn.prepare(
                "SELECT i.id, i.number, i.local_date, p.full_name FROM invoice i JOIN patient p ON p.id = i.patient_id
                 WHERE i.status = 'issued' AND i.deleted_at IS NULL AND i.local_date BETWEEN ?1 AND ?2 ORDER BY i.issued_at",
            )?;
            let invs: Vec<(String, String, String, String)> = stmt
                .query_map([&from, &to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<rusqlite::Result<_>>()?;
            for (id, number, date, patient) in invs {
                let a = amounts(conn, &id)?;
                for l in invoice_lines(&id)? {
                    if doctor_filter.is_some_and(|d| d != l.doctor_id) {
                        continue;
                    }
                    let base = line_net(l.line_total, &a);
                    let commission =
                        commission_of(rate(&rules, &l.doctor_id, l.service_id.as_deref()), base, l.quantity);
                    push(
                        &l.doctor_id,
                        CommissionLine {
                            date: date.clone(),
                            invoice_number: number.clone(),
                            patient_name: patient.clone(),
                            names: l.names,
                            base,
                            commission,
                        },
                        &mut doctor_names,
                    )?;
                }
            }
        }
        CommissionBasis::Payment => {
            // Each payment received (a refund counts negatively) is shared among the invoice's lines
            // by what they earn; the commission follows that share.
            let mut stmt = conn.prepare(
                "SELECT pay.kind, pay.amount, pay.local_date, i.id, i.number, pt.full_name
                 FROM payment pay JOIN invoice i ON i.id = pay.invoice_id JOIN patient pt ON pt.id = pay.patient_id
                 WHERE i.status = 'issued' AND pay.local_date BETWEEN ?1 AND ?2 AND pay.kind IN ('payment', 'credit', 'refund')
                 ORDER BY pay.paid_at",
            )?;
            let pays: Vec<(String, i64, String, String, String, String)> = stmt
                .query_map([&from, &to], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            for (kind, amount, date, id, number, patient) in pays {
                let a = amounts(conn, &id)?;
                if a.net() <= 0 {
                    continue;
                }
                let signed = if kind == "refund" { -amount } else { amount };
                for l in invoice_lines(&id)? {
                    if doctor_filter.is_some_and(|d| d != l.doctor_id) {
                        continue;
                    }
                    let earned = line_net(l.line_total, &a);
                    let base = ((earned as i128) * (signed as i128) / (a.net() as i128)) as i64;
                    let rule = rate(&rules, &l.doctor_id, l.service_id.as_deref());
                    let commission = match rule.map(|r| r.kind) {
                        Some(CommissionKind::Fixed) => {
                            ((commission_of(rule, 0, l.quantity) as i128) * (signed as i128)
                                / (a.net() as i128)) as i64
                        }
                        _ => commission_of(rule, base, l.quantity),
                    };
                    push(
                        &l.doctor_id,
                        CommissionLine {
                            date: date.clone(),
                            invoice_number: number.clone(),
                            patient_name: patient.clone(),
                            names: l.names,
                            base,
                            commission,
                        },
                        &mut doctor_names,
                    )?;
                }
            }
        }
    }
    for (doctor_id, lines) in by_doctor {
        let base: i64 = lines.iter().map(|l| l.base).sum();
        let commission: i64 = lines.iter().map(|l| l.commission).sum();
        report.base_total += base;
        report.commission_total += commission;
        report.doctors.push(DoctorCommission {
            doctor_name: doctor_names.get(&doctor_id).cloned().unwrap_or_default(),
            doctor_id,
            base,
            commission,
            lines,
        });
    }
    report.doctors.sort_by(|a, b| a.doctor_name.cmp(&b.doctor_name));
    Ok(report)
}

// ───────────────────────────── expenses ─────────────────────────────

const CATEGORY_SELECT: &str =
    "SELECT id, name_fa, name_ps, name_en, sort_order, is_system, is_active, version
    FROM expense_category WHERE deleted_at IS NULL";

fn map_category(r: &rusqlite::Row<'_>) -> rusqlite::Result<ExpenseCategoryInfo> {
    Ok(ExpenseCategoryInfo {
        id: r.get(0)?,
        name: trio(r.get(1)?, r.get(2)?, r.get(3)?),
        sort_order: r.get(4)?,
        is_system: r.get::<_, i64>(5)? == 1,
        is_active: r.get::<_, i64>(6)? == 1,
        version: r.get(7)?,
    })
}

pub fn list_categories(conn: &Connection, include_inactive: bool) -> Result<Vec<ExpenseCategoryInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{CATEGORY_SELECT} {} ORDER BY sort_order, name_en",
        if include_inactive { "" } else { "AND is_active = 1" }
    ))?;
    let rows = stmt.query_map([], map_category)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn get_category(conn: &Connection, id: &str) -> Result<ExpenseCategoryInfo> {
    conn.query_row(&format!("{CATEGORY_SELECT} AND id = ?1"), [id], map_category).optional()?.ok_or_else(
        || {
            CoreError::invalid(
                "category_id",
                ValidationRule::ExpenseCategoryNotFound,
                "unknown expense category",
            )
        },
    )
}

pub fn save_category(
    conn: &Connection,
    actor: &Actor,
    p: &SaveExpenseCategoryParams,
) -> Result<ExpenseCategoryInfo> {
    let name = clean_translations("name", &p.name, 60)?;
    let now = now_iso();
    match p.id.as_deref().filter(|s| !s.is_empty()) {
        None => {
            let id = new_id();
            let order =
                scalar(conn, "SELECT MAX(sort_order) FROM expense_category WHERE sort_order < 90", &[])? + 10;
            conn.execute(
                "INSERT INTO expense_category(id, name_fa, name_ps, name_en, sort_order, is_active, created_at, created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?7, ?8)",
                params![id, name.fa, name.ps, name.en, order, p.is_active, now, actor.user_id],
            )?;
            let after = get_category(conn, &id)?;
            audit::record(
                conn,
                actor,
                "expense_category.create",
                Some("expense_category"),
                Some(&id),
                None,
                Some(&json!(after.name)),
            )?;
            Ok(after)
        }
        Some(id) => {
            let before = get_category(conn, id)?;
            let changed = conn.execute(
                "UPDATE expense_category SET name_fa = ?1, name_ps = ?2, name_en = ?3, is_active = ?4, updated_at = ?5,
                    updated_by = ?6, version = version + 1 WHERE id = ?7 AND version = ?8 AND deleted_at IS NULL",
                params![name.fa, name.ps, name.en, p.is_active, now, actor.user_id, id, p.version.unwrap_or(before.version)],
            )?;
            expect_one_row(changed, "expense category")?;
            let after = get_category(conn, id)?;
            audit::record(
                conn,
                actor,
                "expense_category.update",
                Some("expense_category"),
                Some(id),
                Some(&json!(before)),
                Some(&json!(after)),
            )?;
            Ok(after)
        }
    }
}

fn day_closed(conn: &Connection, date: &str) -> Result<bool> {
    Ok(scalar(conn, "SELECT COUNT(*) FROM cash_closing WHERE local_date = ?1", &[&date])? > 0)
}

const EXPENSE_SELECT: &str =
    "SELECT e.id, e.category_id, c.name_fa, c.name_ps, c.name_en, e.amount, e.local_date, e.description,
    e.payee, e.from_cash, u.display_name, e.version
    FROM expense e JOIN expense_category c ON c.id = e.category_id LEFT JOIN app_user u ON u.id = e.created_by
    WHERE e.deleted_at IS NULL";

fn map_expense(r: &rusqlite::Row<'_>) -> rusqlite::Result<ExpenseInfo> {
    Ok(ExpenseInfo {
        id: r.get(0)?,
        category_id: r.get(1)?,
        category: trio(r.get(2)?, r.get(3)?, r.get(4)?),
        amount: r.get(5)?,
        local_date: r.get(6)?,
        description: r.get(7)?,
        payee: r.get(8)?,
        from_cash: r.get::<_, i64>(9)? == 1,
        created_by_name: r.get(10)?,
        version: r.get(11)?,
    })
}

pub fn list_expenses(conn: &Connection, p: &ExpenseListParams) -> Result<Vec<ExpenseInfo>> {
    let to = opt_date("to", &p.to)?.unwrap_or_else(today_iso);
    let from = match opt_date("from", &p.from)? {
        Some(f) => f,
        None => format_date(parse_date("to", &to)? - time::Duration::days(30)),
    };
    let mut stmt = conn.prepare(&format!(
        "{EXPENSE_SELECT} AND e.local_date BETWEEN ?1 AND ?2 ORDER BY e.local_date DESC, e.created_at DESC LIMIT 1000"
    ))?;
    let rows = stmt.query_map([&from, &to], map_expense)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn get_expense(conn: &Connection, id: &str) -> Result<ExpenseInfo> {
    conn.query_row(&format!("{EXPENSE_SELECT} AND e.id = ?1"), [id], map_expense).optional()?.ok_or_else(
        || {
            CoreError::api(ErrorCode::NotFound, format!("expense {id}"))
                .on_field("id", ValidationRule::ExpenseNotFound)
        },
    )
}

pub fn add_expense(conn: &Connection, actor: &Actor, p: &AddExpenseParams) -> Result<ExpenseInfo> {
    let category = get_category(conn, &p.category_id)?;
    let amount = money("amount", p.amount, false)?;
    let date = date_of("date", &p.date)?;
    if date > today_iso() {
        return Err(CoreError::invalid(
            "date",
            ValidationRule::DateFormat,
            "an expense cannot be dated in the future",
        ));
    }
    if p.from_cash && day_closed(conn, &date)? {
        return Err(CoreError::invalid(
            "date",
            ValidationRule::CashDayClosed,
            "the cash of that day is already closed",
        ));
    }
    let description = required("description", &p.description, 200)?;
    let payee = optional("payee", &p.payee, 100)?;
    let id = new_id();
    let now = now_iso();
    conn.execute(
        "INSERT INTO expense(id, category_id, amount, local_date, description, payee, from_cash, created_at, created_by, updated_at, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?8, ?9)",
        params![id, category.id, amount, date, description, payee, p.from_cash, now, actor.user_id],
    )?;
    let after = get_expense(conn, &id)?;
    audit::record(
        conn,
        actor,
        "expense.add",
        Some("expense"),
        Some(&id),
        None,
        Some(
            &json!({"amount": amount, "date": date, "category": category.name.en, "description": description}),
        ),
    )?;
    Ok(after)
}

pub fn delete_expense(conn: &Connection, actor: &Actor, p: &DeleteExpenseParams) -> Result<()> {
    let before = get_expense(conn, &p.id)?;
    let reason = required("reason", &p.reason, 300)?;
    if before.from_cash && day_closed(conn, &before.local_date)? {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::CashDayClosed,
            "the cash of that day is already closed",
        ));
    }
    let now = now_iso();
    let changed = conn.execute(
        "UPDATE expense SET deleted_at = ?1, delete_reason = ?2, updated_at = ?1, updated_by = ?3, version = version + 1
         WHERE id = ?4 AND version = ?5 AND deleted_at IS NULL",
        params![now, reason, actor.user_id, p.id, p.version],
    )?;
    expect_one_row(changed, "expense")?;
    audit::record(
        conn,
        actor,
        "expense.delete",
        Some("expense"),
        Some(&p.id),
        Some(&json!({"amount": before.amount, "date": before.local_date, "description": before.description})),
        Some(&json!({"reason": reason})),
    )?;
    Ok(())
}

// ───────────────────────────── cash closing & summary ─────────────────────────────

fn by_method(conn: &Connection, from: &str, to: &str) -> Result<Vec<MethodTotal>> {
    let mut stmt = conn.prepare(
        "SELECT m.name_fa, m.name_ps, m.name_en, m.is_cash,
            COALESCE(SUM(CASE WHEN p.kind = 'payment' THEN p.amount ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN p.kind = 'refund' AND p.returns_to_credit = 0 THEN p.amount ELSE 0 END), 0)
         FROM payment p JOIN payment_method m ON m.id = p.method_id
         WHERE p.local_date BETWEEN ?1 AND ?2 AND p.kind IN ('payment', 'refund')
         GROUP BY m.id ORDER BY m.sort_order",
    )?;
    let rows = stmt
        .query_map([from, to], |r| {
            Ok(MethodTotal {
                method: trio(r.get(0)?, r.get(1)?, r.get(2)?),
                is_cash: r.get::<_, i64>(3)? == 1,
                received: r.get(4)?,
                refunded: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn closing_of(conn: &Connection, date: &str) -> Result<Option<CashClosingInfo>> {
    Ok(conn
        .query_row(
            "SELECT c.id, c.local_date, c.expected, c.counted, c.difference, c.reason, c.closed_at, u.display_name
             FROM cash_closing c LEFT JOIN app_user u ON u.id = c.closed_by WHERE c.local_date = ?1",
            [date],
            map_closing,
        )
        .optional()?)
}

fn map_closing(r: &rusqlite::Row<'_>) -> rusqlite::Result<CashClosingInfo> {
    Ok(CashClosingInfo {
        id: r.get(0)?,
        date: r.get(1)?,
        expected: r.get(2)?,
        counted: r.get(3)?,
        difference: r.get(4)?,
        reason: r.get(5)?,
        closed_at: r.get(6)?,
        closed_by_name: r.get(7)?,
    })
}

pub fn cash_day(conn: &Connection, date: &str) -> Result<CashDay> {
    let date = date_of("date", date)?;
    let by_method = by_method(conn, &date, &date)?;
    let cash_in = by_method.iter().filter(|m| m.is_cash).map(|m| m.received).sum();
    let cash_refunds = by_method.iter().filter(|m| m.is_cash).map(|m| m.refunded).sum();
    let cash_expenses = scalar(
        conn,
        "SELECT SUM(amount) FROM expense WHERE local_date = ?1 AND from_cash = 1 AND deleted_at IS NULL",
        &[&date],
    )?;
    let payment_count =
        scalar(conn, "SELECT COUNT(*) FROM payment WHERE local_date = ?1 AND kind = 'payment'", &[&date])?;
    Ok(CashDay {
        closing: closing_of(conn, &date)?,
        expected: cash_in - cash_refunds - cash_expenses,
        by_method,
        cash_in,
        cash_refunds,
        cash_expenses,
        payment_count,
        date,
    })
}

pub fn close_cash(conn: &Connection, actor: &Actor, p: &CloseCashParams) -> Result<CashClosingInfo> {
    let date = date_of("date", &p.date)?;
    if date > today_iso() {
        return Err(CoreError::invalid(
            "date",
            ValidationRule::DateFormat,
            "a day cannot be closed before it happens",
        ));
    }
    if day_closed(conn, &date)? {
        return Err(CoreError::invalid("date", ValidationRule::CashDayClosed, "this day is already closed"));
    }
    let counted = money("counted", p.counted, true)?;
    let day = cash_day(conn, &date)?;
    let difference = counted - day.expected;
    let reason = optional("reason", &p.reason, 300)?;
    if difference != 0 && reason.is_none() {
        return Err(CoreError::invalid(
            "reason",
            ValidationRule::CashReasonRequired,
            "write why the count differs from the expected amount",
        ));
    }
    let id = new_id();
    conn.execute(
        "INSERT INTO cash_closing(id, local_date, expected, counted, difference, reason, closed_at, closed_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, date, day.expected, counted, difference, reason, now_iso(), actor.user_id],
    )?;
    let after =
        closing_of(conn, &date)?.ok_or_else(|| CoreError::api(ErrorCode::Internal, "closing not saved"))?;
    audit::record(
        conn,
        actor,
        "cash.close",
        Some("cash_closing"),
        Some(&id),
        None,
        Some(
            &json!({"date": date, "expected": day.expected, "counted": counted, "difference": difference, "reason": reason}),
        ),
    )?;
    Ok(after)
}

pub fn list_closings(conn: &Connection, p: &ExpenseListParams) -> Result<Vec<CashClosingInfo>> {
    let to = opt_date("to", &p.to)?.unwrap_or_else(today_iso);
    let from = match opt_date("from", &p.from)? {
        Some(f) => f,
        None => format_date(parse_date("to", &to)? - time::Duration::days(60)),
    };
    let mut stmt = conn.prepare(
        "SELECT c.id, c.local_date, c.expected, c.counted, c.difference, c.reason, c.closed_at, u.display_name
         FROM cash_closing c LEFT JOIN app_user u ON u.id = c.closed_by
         WHERE c.local_date BETWEEN ?1 AND ?2 ORDER BY c.local_date DESC",
    )?;
    let rows = stmt.query_map([&from, &to], map_closing)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn summary(conn: &Connection, p: &PeriodParams) -> Result<FinanceSummary> {
    let from = date_of("from", &p.from)?;
    let to = date_of("to", &p.to)?;
    if from > to {
        return Err(CoreError::invalid("to", ValidationRule::PeriodRange, "the end is before the start"));
    }
    let by_method = by_method(conn, &from, &to)?;
    let received: i64 = by_method.iter().map(|m| m.received).sum();
    let refunded: i64 = by_method.iter().map(|m| m.refunded).sum();
    let invoiced = scalar(
        conn,
        "SELECT COALESCE(SUM((SELECT COALESCE(SUM(line_total), 0) FROM invoice_line WHERE invoice_id = i.id) - i.discount
                - (SELECT COALESCE(SUM(amount), 0) FROM credit_note WHERE invoice_id = i.id)), 0)
         FROM invoice i WHERE i.status = 'issued' AND i.deleted_at IS NULL AND i.local_date BETWEEN ?1 AND ?2",
        &[&from, &to],
    )?;
    let mut stmt = conn.prepare(
        "SELECT c.name_fa, c.name_ps, c.name_en, SUM(e.amount) FROM expense e JOIN expense_category c ON c.id = e.category_id
         WHERE e.deleted_at IS NULL AND e.local_date BETWEEN ?1 AND ?2 GROUP BY c.id ORDER BY SUM(e.amount) DESC",
    )?;
    let by_category: Vec<CategoryTotal> = stmt
        .query_map([&from, &to], |r| {
            Ok(CategoryTotal { category: trio(r.get(0)?, r.get(1)?, r.get(2)?), amount: r.get(3)? })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let expenses = by_category.iter().map(|c| c.amount).sum();
    Ok(FinanceSummary {
        from,
        to,
        invoiced,
        received,
        refunded,
        expenses,
        profit: received - refunded - expenses,
        by_method,
        by_category,
    })
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
    fn the_most_specific_commission_rule_wins() {
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
        assert_eq!(rate(&rules, "d2", None).unwrap().value, 1000);
        assert!(rate(&[], "d1", None).is_none());
    }

    #[test]
    fn commission_is_a_percent_of_the_base_or_a_fixed_sum_per_unit() {
        let pct = rule(None, None, CommissionKind::Percent, 2500);
        assert_eq!(commission_of(Some(&pct), 400_000, 1), 100_000);
        let fixed = rule(None, None, CommissionKind::Fixed, 20_000);
        assert_eq!(commission_of(Some(&fixed), 400_000, 3), 60_000);
        assert_eq!(commission_of(None, 400_000, 1), 0);
    }

    #[test]
    fn a_line_earns_its_share_of_the_invoice_net() {
        let a = Amounts { subtotal: 1_000_000, discount: 100_000, credit_notes: 0, paid: 0, refunded: 0 };
        assert_eq!(line_net(500_000, &a), 450_000);
        assert_eq!(line_net(500_000, &Amounts::default()), 0);
    }

    #[test]
    fn amounts_never_go_negative() {
        let a = Amounts { subtotal: 100, discount: 150, credit_notes: 0, paid: 0, refunded: 40 };
        assert_eq!((a.net(), a.applied(), a.remaining()), (0, 0, 0));
        let b = Amounts { subtotal: 1000, discount: 100, credit_notes: 100, paid: 500, refunded: 100 };
        assert_eq!((b.net(), b.applied(), b.remaining()), (800, 400, 400));
    }
}
