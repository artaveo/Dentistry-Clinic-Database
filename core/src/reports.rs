//! Phase 9 — dashboard, reports, notifications and global search (roadmap 9.1–9.7). Everything here only
//! reads what the earlier phases wrote, through the indexes they made (plus the few added in migration 17),
//! so nothing scans a whole table for a screen the user opens many times a day.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::time::Instant;

use artaveo_shared::*;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::json;
use time::{PrimitiveDateTime, Time};

use crate::audit::{self, Actor};
use crate::auth::perm;
use crate::calendar::GregorianDate;
use crate::clock::{iso, now, now_iso, parse, today_iso, CLINIC_OFFSET};
use crate::error::{CoreError, Result};
use crate::fin::ledger::NUMBER_SQL;
use crate::fin::util::{iso_of, ymd};
use crate::fin::{self, date_of, scalar, trio};
use crate::inventory;
use crate::patient;
use crate::scheduling::{format_date, format_minutes, parse_date};
use crate::session::Session;

/// Rows one report shows; more are cut and the screen says so.
const MAX_ROWS: usize = 5000;
/// A backup older than this many hours is "overdue".
const BACKUP_OVERDUE_HOURS: i64 = 36;

// ───────────────────────────── shared helpers ─────────────────────────────

/// UTC instant of the start of a clinic-local day.
pub(crate) fn day_start_utc(field: &'static str, date: &str) -> Result<String> {
    let d = parse_date(field, date)?;
    Ok(iso(PrimitiveDateTime::new(d, Time::MIDNIGHT).assume_offset(CLINIC_OFFSET)))
}

/// `[start of from, start of the day after to)` as UTC instants, for columns that hold timestamps.
pub(crate) fn utc_bounds(from: &str, to: &str) -> Result<(String, String)> {
    let next = format_date(parse_date("to", to)? + time::Duration::days(1));
    Ok((day_start_utc("from", from)?, day_start_utc("to", &next)?))
}

fn period(from: &str, to: &str) -> Result<(String, String)> {
    let from = date_of("from", from)?;
    let to = date_of("to", to)?;
    if from > to {
        return Err(CoreError::invalid(
            "to",
            ValidationRule::PeriodRange,
            "the end of the period is before its start",
        ));
    }
    Ok((from, to))
}

fn text(s: impl Into<String>) -> ReportCell {
    ReportCell::Text { value: s.into() }
}
fn code(s: impl Into<String>) -> ReportCell {
    ReportCell::Code { value: s.into() }
}
fn tag(s: impl Into<String>) -> ReportCell {
    ReportCell::Tag { value: s.into() }
}
fn date(s: impl Into<String>) -> ReportCell {
    ReportCell::Date { value: s.into() }
}
fn int(v: i64) -> ReportCell {
    ReportCell::Int { value: v }
}
fn money(v: i64) -> ReportCell {
    ReportCell::Money { value: v }
}
fn qty(v: i64) -> ReportCell {
    ReportCell::Qty { value: v }
}
fn name(fa: String, ps: String, en: String) -> ReportCell {
    ReportCell::Name { value: trio(fa, ps, en) }
}
fn opt_text(s: Option<String>) -> ReportCell {
    match s.filter(|s| !s.is_empty()) {
        Some(s) => ReportCell::Text { value: s },
        None => ReportCell::Empty,
    }
}
fn opt_code(s: Option<String>) -> ReportCell {
    match s.filter(|s| !s.is_empty()) {
        Some(s) => ReportCell::Code { value: s },
        None => ReportCell::Empty,
    }
}
fn opt_date(s: Option<String>) -> ReportCell {
    match s.filter(|s| !s.is_empty()) {
        Some(s) => ReportCell::Date { value: s },
        None => ReportCell::Empty,
    }
}
/// The date part of a stored UTC timestamp (a report row needs a day, not a minute).
fn day_of(ts: &str) -> String {
    ts.chars().take(10).collect()
}

struct Table {
    cols: Vec<&'static str>,
    rows: Vec<Vec<ReportCell>>,
    totals: Vec<ReportCell>,
}

/// Adds a total to every column in `sum` (money, counts and quantities); other columns stay empty.
fn table(cols: &[&'static str], rows: Vec<Vec<ReportCell>>, sum: &[usize]) -> Table {
    let mut totals = Vec::new();
    if !rows.is_empty() && !sum.is_empty() {
        for i in 0..cols.len() {
            if !sum.contains(&i) {
                totals.push(ReportCell::Empty);
                continue;
            }
            let (mut total, mut kind) = (0i64, 0u8);
            for row in &rows {
                match row.get(i) {
                    Some(ReportCell::Money { value }) => {
                        total += value;
                        kind = 1;
                    }
                    Some(ReportCell::Qty { value }) => {
                        total += value;
                        kind = 2;
                    }
                    Some(ReportCell::Int { value }) => total += value,
                    _ => {}
                }
            }
            totals.push(match kind {
                1 => money(total),
                2 => qty(total),
                _ => int(total),
            });
        }
    }
    Table { cols: cols.to_vec(), rows, totals }
}

/// `YYYY-MM` of a day in the clinic's calendar (a Shamsi clinic reads months, not Gregorian ones).
fn month_key(day: &str, calendar: CalendarSystem) -> String {
    let plain = || day.chars().take(7).collect::<String>();
    if calendar == CalendarSystem::Gregorian {
        return plain();
    }
    let mut it = day.split('-').map(|x| x.parse::<i32>().unwrap_or(0));
    let (y, m, d) = (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0));
    match (u8::try_from(m), u8::try_from(d)) {
        (Ok(month), Ok(day)) => GregorianDate { year: y, month, day }
            .to_shamsi()
            .map(|s| format!("{:04}-{:02}", s.year, s.month))
            .unwrap_or_else(|_| plain()),
        _ => plain(),
    }
}

// ───────────────────────────── dashboard (9.1) ─────────────────────────────

/// The doctor row of a user who is a doctor and may not see the clinic's reports (Owner and
/// administrators see everything even when they are doctors too).
fn own_doctor(conn: &Connection, s: &Session) -> Result<Option<(String, String)>> {
    if s.permissions.contains(perm::REPORTS_VIEW) {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            "SELECT id, full_name FROM doctor WHERE user_id = ?1 AND deleted_at IS NULL",
            [&s.user.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

fn backup_status(conn: &Connection) -> Result<BackupStatus> {
    let last: Option<(String, i64)> = conn
        .query_row(
            "SELECT created_at, verified FROM backup_log WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let since = last.as_ref().map(|l| l.0.clone()).unwrap_or_default();
    let failed_since =
        scalar(conn, "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.failed' AND at > ?1", &[&since])?
            > 0;
    let age_hours = last.as_ref().and_then(|l| parse(&l.0)).map(|t| (now() - t).whole_hours());
    Ok(BackupStatus {
        last_at: last.as_ref().map(|l| l.0.clone()),
        last_verified: last.as_ref().is_some_and(|l| l.1 == 1),
        age_hours,
        failed_since,
    })
}

/// `license` is the license state code (`active`, `trial`, ...), known to the caller, not to the database.
pub fn dashboard(conn: &Connection, s: &Session, license: Option<String>) -> Result<Dashboard> {
    let today = today_iso();
    let can = |p: &str| s.permissions.contains(p);
    let doctor = own_doctor(conn, s)?;
    let doctor_id: Option<String> = doctor.as_ref().map(|d| d.0.clone());
    let scope = if doctor.is_some() { DashboardScope::Doctor } else { DashboardScope::Clinic };

    let (patients_today, appointments_today, appointments_done) = if can(perm::APPOINTMENTS_VIEW) {
        conn.query_row(
            "SELECT COUNT(DISTINCT patient_id), COUNT(*), COALESCE(SUM(status = 'completed'), 0) FROM appointment
             WHERE deleted_at IS NULL AND local_date = ?1 AND status NOT IN ('cancelled', 'no_show', 'rescheduled')
               AND (?2 IS NULL OR doctor_id = ?2)",
            params![today, doctor_id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
        )?
    } else {
        (0, 0, 0)
    };

    let (lo, hi) = utc_bounds(&today, &today)?;
    let treatments_completed = scalar(
        conn,
        "SELECT COUNT(*) FROM treatment_item WHERE deleted_at IS NULL AND status = 'completed'
           AND completed_at >= ?1 AND completed_at < ?2 AND (?3 IS NULL OR doctor_id = ?3)",
        &[&lo, &hi, &doctor_id],
    )?;

    let (revenue_basis, revenue_today, outstanding) = if !can(perm::BILLING_VIEW) {
        (RevenueBasis::Received, None, None)
    } else if let Some(id) = &doctor_id {
        let own = scalar(
            conn,
            "SELECT SUM(l.amount - COALESCE(l.discount, 0)) FROM fin_line l JOIN fin_doc d ON d.id = l.doc_id
             WHERE d.kind = 'inv' AND d.status = 'posted' AND d.doc_date = ?1 AND l.provider_id = ?2",
            &[&ymd(&today), id],
        )?;
        (RevenueBasis::Invoiced, Some(own), None)
    } else {
        // Money from patients today (refunds taken off), and everything patients owe on invoices.
        let received = scalar(
            conn,
            "SELECT SUM(g.amount) FROM fin_gl g JOIN fin_doc d ON d.id = g.doc_id JOIN fin_account a ON a.id = g.account_id
             WHERE g.date = ?1 AND d.kind IN ('rc', 'rf') AND a.money IS NOT NULL",
            &[&ymd(&today)],
        )?;
        let owed = scalar(conn, "SELECT SUM(amount) FROM fin_gl WHERE account_id = 1310", &[])?;
        (RevenueBasis::Received, Some(received), Some(owed))
    };

    let mut upcoming = Vec::new();
    if can(perm::APPOINTMENTS_VIEW) {
        let mut stmt = conn.prepare(
            "SELECT a.id, a.patient_id, p.full_name, d.full_name, a.local_date, a.start_min, a.status
             FROM appointment a JOIN patient p ON p.id = a.patient_id JOIN doctor d ON d.id = a.doctor_id
             WHERE a.deleted_at IS NULL AND a.local_date >= ?1 AND a.start_at >= ?2
               AND a.status IN ('scheduled', 'confirmed', 'checked_in') AND (?3 IS NULL OR a.doctor_id = ?3)
             ORDER BY a.start_at LIMIT 8",
        )?;
        upcoming = stmt
            .query_map(params![today, now_iso(), doctor_id], |r| {
                Ok(DashboardVisit {
                    appointment_id: r.get(0)?,
                    patient_id: r.get(1)?,
                    patient_name: r.get(2)?,
                    doctor_name: r.get(3)?,
                    date: r.get(4)?,
                    time: format_minutes(r.get(5)?),
                    status: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
    }

    let low_stock = if can(perm::INVENTORY_VIEW) && doctor.is_none() {
        let low = inventory::list_items(
            conn,
            &StockItemListParams { only: Some(StockFilter::Low), ..Default::default() },
        )?;
        Some(low.len() as i64)
    } else {
        None
    };
    let pending_lab = if can(perm::INVENTORY_VIEW) {
        Some(scalar(
            conn,
            "SELECT COUNT(*) FROM lab_case WHERE status IN ('planned', 'sent') AND (?1 IS NULL OR doctor_id = ?1)",
            &[&doctor_id],
        )?)
    } else {
        None
    };
    let backup = if can(perm::BACKUP_VIEW) { Some(backup_status(conn)?) } else { None };

    Ok(Dashboard {
        date: today,
        scope,
        doctor_name: doctor.map(|d| d.1),
        patients_today,
        appointments_today,
        appointments_done,
        treatments_completed,
        revenue_basis,
        revenue_today,
        outstanding,
        upcoming,
        low_stock,
        pending_lab,
        backup,
        license,
    })
}

// ───────────────────────────── reports (9.2–9.5) ─────────────────────────────

struct Ctx<'a> {
    conn: &'a Connection,
    from: String,
    to: String,
    /// UTC bounds of the period for timestamp columns.
    lo: String,
    hi: String,
    doctor: Option<String>,
    service: Option<String>,
    /// The money account of a money book or the cash counts.
    account: Option<i64>,
    calendar: CalendarSystem,
}

pub fn run(conn: &Connection, p: &ReportParams) -> Result<ReportResult> {
    let started = Instant::now();
    let (from, to) = period(&p.from, &p.to)?;
    let (lo, hi) = utc_bounds(&from, &to)?;
    let clean = |s: &Option<String>| s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    let c = Ctx {
        conn,
        from: from.clone(),
        to: to.clone(),
        lo,
        hi,
        doctor: clean(&p.doctor_id),
        service: clean(&p.service_id),
        account: p.account_id,
        calendar: p.calendar.unwrap_or_default(),
    };
    use ReportKind::*;
    let mut t = match p.kind {
        DailySummary => daily_summary(&c)?,
        MonthlySummary => monthly_summary(&c)?,
        Receipts => receipts_register(&c)?,
        Invoices => invoices_register(&c)?,
        Adjustments => adjustments(&c)?,
        Aging => aging(&c)?,
        Installments => installments(&c)?,
        Deposits => deposits(&c)?,
        Expenses => expenses(&c)?,
        MoneyBook => money_book(&c)?,
        ProviderProduction => provider_production(&c)?,
        ServiceRevenue => service_revenue(&c)?,
        DoctorShares => doctor_shares(&c)?,
        CashCounts => cash_counts(&c)?,
        IncomeStatement => income_statement(&c)?,
        Payroll => payroll(&c)?,
        Withholding => withholding(&c)?,
        Patients => patients(&c)?,
        Treatments => treatments(&c)?,
        Procedures => procedures(&c)?,
        ToothConditions => tooth_conditions(&c)?,
        DoctorActivity => doctor_activity(&c)?,
        FollowUps => follow_ups(&c)?,
        RecallList => recall_list(&c)?,
        ReferralSources => referral_sources(&c)?,
        Stock => stock(&c)?,
        LowStock => low_stock(&c)?,
        Purchases => purchases(&c)?,
        Consumption => consumption(&c)?,
        Expiry => expiry(&c)?,
        SupplierBalance => supplier_balance(&c)?,
    };
    let truncated = t.rows.len() > MAX_ROWS;
    if truncated {
        t.rows.truncate(MAX_ROWS);
        t.totals.clear();
    }
    Ok(ReportResult {
        kind: p.kind,
        from,
        to,
        columns: t.cols.iter().map(|k| ReportColumn { key: (*k).to_string() }).collect(),
        rows: t.rows,
        totals: t.totals,
        truncated,
        millis: started.elapsed().as_millis().min(u32::MAX as u128) as u32,
    })
}

type Rows = Vec<Vec<ReportCell>>;

fn collect(
    c: &Ctx,
    sql: &str,
    args: &[&dyn rusqlite::ToSql],
    f: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<Vec<ReportCell>>,
) -> Result<Rows> {
    let mut stmt = c.conn.prepare(sql)?;
    let rows = stmt.query_map(args, f)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

const CAP: &str = "LIMIT 5001";

// ───────────────────────────── finance reports (docs/design/finance-standard.md §ب۹) ─────────────────────────────

/// The period as `yyyymmdd` integers.
fn days(c: &Ctx) -> (i64, i64) {
    (ymd(&c.from), ymd(&c.to))
}

/// An account in a report: built-in accounts by code (the screen names them), the clinic's own by name.
fn account_cell(conn: &Connection, id: Option<i64>) -> Result<ReportCell> {
    let Some(id) = id else { return Ok(ReportCell::Empty) };
    let name: Option<String> = conn
        .query_row("SELECT name FROM fin_account WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .flatten();
    Ok(match name {
        Some(n) => text(n),
        None => tag(format!("acct_{id}")),
    })
}

fn status_tag(status: &str) -> ReportCell {
    tag(match status {
        "void" => "void",
        "superseded" => "superseded",
        _ => "posted",
    })
}

/// Invoiced (accrual) and received (cash) on each day of the period.
#[derive(Default, Clone, Copy)]
struct DayMoney {
    invoices: i64,
    gross: i64,
    discounts: i64,
    reductions: i64,
    receipts: i64,
    received: i64,
    refunded: i64,
}

fn day_money(c: &Ctx) -> Result<BTreeMap<i64, DayMoney>> {
    let (from, to) = days(c);
    let mut out: BTreeMap<i64, DayMoney> = BTreeMap::new();
    let mut stmt = c.conn.prepare(
        "SELECT g.date, d.kind, g.account_id, a.money IS NOT NULL, SUM(g.amount)
         FROM fin_gl g JOIN fin_doc d ON d.id = g.doc_id JOIN fin_account a ON a.id = g.account_id
         WHERE g.date BETWEEN ?1 AND ?2 AND (d.kind IN ('rc', 'rf') OR g.account_id IN (4110, 4910, 4920))
         GROUP BY g.date, d.kind, g.account_id",
    )?;
    let rows: Vec<(i64, String, i64, bool, i64)> = stmt
        .query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (day, kind, account, is_money, amount) in rows {
        let e = out.entry(day).or_default();
        match (kind.as_str(), account, is_money) {
            (_, 4110, _) => e.gross -= amount,
            (_, 4910, _) => e.discounts += amount,
            (_, 4920, _) => e.reductions += amount,
            ("rc", _, true) => e.received += amount,
            ("rf", _, true) => e.refunded -= amount,
            _ => {}
        }
    }
    let mut stmt = c.conn.prepare(
        "SELECT doc_date, SUM(kind = 'inv'), SUM(kind = 'rc') FROM fin_doc
         WHERE doc_date BETWEEN ?1 AND ?2 AND kind IN ('inv', 'rc') AND status = 'posted' GROUP BY doc_date",
    )?;
    for row in stmt.query_map(params![from, to], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
    })? {
        let (day, invoices, receipts) = row?;
        let e = out.entry(day).or_default();
        e.invoices = invoices;
        e.receipts = receipts;
    }
    Ok(out)
}

fn summary_row(first: ReportCell, m: &DayMoney) -> Vec<ReportCell> {
    vec![
        first,
        int(m.invoices),
        money(m.gross),
        money(m.discounts + m.reductions),
        money(m.gross - m.discounts - m.reductions),
        int(m.receipts),
        money(m.received),
        money(m.refunded),
        money(m.received - m.refunded),
    ]
}

const SUMMARY_COLS: [&str; 8] =
    ["invoices", "gross", "discounts", "invoiced", "receipts", "received", "refunded", "net_received"];

fn daily_summary(c: &Ctx) -> Result<Table> {
    let rows = day_money(c)?.iter().map(|(d, m)| summary_row(date(iso_of(*d)), m)).collect();
    let mut cols = vec!["date"];
    cols.extend(SUMMARY_COLS);
    Ok(table(&cols, rows, &[1, 2, 3, 4, 5, 6, 7, 8]))
}

fn monthly_summary(c: &Ctx) -> Result<Table> {
    let mut months: BTreeMap<String, DayMoney> = BTreeMap::new();
    for (d, m) in day_money(c)? {
        let e = months.entry(month_key(&iso_of(d), c.calendar)).or_default();
        e.invoices += m.invoices;
        e.gross += m.gross;
        e.discounts += m.discounts;
        e.reductions += m.reductions;
        e.receipts += m.receipts;
        e.received += m.received;
        e.refunded += m.refunded;
    }
    let rows = months.into_iter().map(|(k, m)| summary_row(ReportCell::Month { value: k }, &m)).collect();
    let mut cols = vec!["month"];
    cols.extend(SUMMARY_COLS);
    Ok(table(&cols, rows, &[1, 2, 3, 4, 5, 6, 7, 8]))
}

fn receipts_register(c: &Ctx) -> Result<Table> {
    let (from, to) = days(c);
    let mut stmt = c.conn.prepare(&format!(
        "SELECT d.doc_date, {NUMBER_SQL}, p.patient_number, p.full_name, d.kind, d.account_id, d.amount, d.status,
            COALESCE(d.reason, d.name, '')
         FROM fin_doc d JOIN patient p ON p.id = d.party_id
         WHERE d.kind IN ('rc', 'rf') AND d.doc_date BETWEEN ?1 AND ?2 ORDER BY d.doc_date, d.kind, d.serial {CAP}"
    ))?;
    type R = (i64, String, String, String, String, Option<i64>, i64, String, String);
    let raw: Vec<R> = stmt
        .query_map(params![from, to], |r| {
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
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut rows = Vec::with_capacity(raw.len());
    for (d, number, pno, patient, kind, account, amount, status, note) in raw {
        let posted = status == "posted";
        let (received, refunded) = match (kind.as_str(), posted) {
            ("rc", true) => (amount, 0),
            ("rf", true) => (0, amount),
            _ => (0, 0),
        };
        rows.push(vec![
            date(iso_of(d)),
            code(number),
            code(pno),
            text(patient),
            account_cell(c.conn, account)?,
            money(received),
            money(refunded),
            status_tag(&status),
            opt_text(Some(note)),
        ]);
    }
    Ok(table(
        &["date", "number", "patient_number", "patient", "account", "received", "refunded", "status", "note"],
        rows,
        &[5, 6],
    ))
}

fn invoices_register(c: &Ctx) -> Result<Table> {
    let (from, to) = days(c);
    let mut stmt = c.conn.prepare(&format!(
        "SELECT d.id, d.doc_date, {NUMBER_SQL}, p.patient_number, p.full_name, d.status, d.amount,
            (SELECT COALESCE(SUM(l.amount), 0) FROM fin_line l WHERE l.doc_id = d.id)
         FROM fin_doc d JOIN patient p ON p.id = d.party_id
         WHERE d.kind = 'inv' AND d.doc_date BETWEEN ?1 AND ?2 ORDER BY d.serial {CAP}"
    ))?;
    type R = (i64, i64, String, String, String, String, i64, i64);
    let raw: Vec<R> = stmt
        .query_map(params![from, to], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut rows = Vec::with_capacity(raw.len());
    for (id, d, number, pno, patient, status, net, gross) in raw {
        let posted = status == "posted";
        let s = fin::invoices::settled(c.conn, id)?;
        let z = |v: i64| money(if posted { v } else { 0 });
        rows.push(vec![
            date(iso_of(d)),
            code(number),
            code(pno),
            text(patient),
            z(gross),
            z(gross - net),
            z(net),
            z(s.reductions + s.written_off),
            z(s.allocated),
            z(s.remaining()),
            status_tag(&status),
        ]);
    }
    Ok(table(
        &[
            "date",
            "number",
            "patient_number",
            "patient",
            "gross",
            "discount",
            "net",
            "reductions",
            "allocated",
            "remaining",
            "status",
        ],
        rows,
        &[4, 5, 6, 7, 8, 9],
    ))
}

fn adjustments(c: &Ctx) -> Result<Table> {
    let (from, to) = days(c);
    let mut stmt = c.conn.prepare(&format!(
        "SELECT d.doc_date, {NUMBER_SQL}, d.adj, p.full_name, d.adj_amount, d.adj_reason, u.display_name FROM (
             SELECT d.id, d.doc_date, d.kind, d.year, d.serial, d.party_id, d.created_by,
                    CASE WHEN d.kind = 'inv' THEN (SELECT COALESCE(SUM(l.discount), 0) FROM fin_line l WHERE l.doc_id = d.id)
                         WHEN d.kind = 'al' THEN -(SELECT COALESCE(SUM(l.amount), 0) FROM fin_line l WHERE l.doc_id = d.id
                            AND l.amount < 0)
                         ELSE d.amount END AS adj_amount,
                    CASE WHEN d.kind = 'inv' THEN 'discount' WHEN d.kind = 'al' THEN 'back' ELSE d.kind END AS adj,
                    COALESCE(d.reason, d.note) AS adj_reason
             FROM fin_doc d WHERE d.status = 'posted' AND d.doc_date BETWEEN ?1 AND ?2 AND d.party = 'patient'
                AND d.kind IN ('inv', 'cn', 'wo', 'rf', 'al')
         ) d JOIN patient p ON p.id = d.party_id LEFT JOIN app_user u ON u.id = d.created_by
         WHERE d.adj_amount <> 0 ORDER BY d.doc_date, d.id {CAP}"
    ))?;
    let rows = stmt
        .query_map(params![from, to], |r| {
            Ok(vec![
                date(iso_of(r.get(0)?)),
                code(r.get::<_, String>(1)?),
                tag(r.get::<_, String>(2)?),
                text(r.get::<_, String>(3)?),
                money(r.get(4)?),
                opt_text(r.get(5)?),
                opt_text(r.get(6)?),
            ])
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(table(&["date", "number", "kind", "patient", "amount", "reason", "user"], rows, &[4]))
}

fn aging(c: &Ctx) -> Result<Table> {
    let rows = fin::patients::aging(
        c.conn,
        &AgingParams { as_of: Some(c.to.clone()), search: None, limit: Some(5000) },
    )?
    .into_iter()
    .map(|a| {
        vec![
            code(a.patient_number),
            text(a.patient_name),
            opt_code(a.phone),
            int(a.invoice_count),
            money(a.d0_30),
            money(a.d31_60),
            money(a.d61_90),
            money(a.d90_plus),
            money(a.total),
            money(a.deposit),
            opt_date(a.oldest_date),
        ]
    })
    .collect();
    Ok(table(
        &[
            "patient_number",
            "patient",
            "phone",
            "invoices",
            "d0_30",
            "d31_60",
            "d61_90",
            "d90_plus",
            "total",
            "deposit",
            "oldest",
        ],
        rows,
        &[3, 4, 5, 6, 7, 8, 9],
    ))
}

fn installments(c: &Ctx) -> Result<Table> {
    let rows = fin::patients::due(
        c.conn,
        &InstallmentsDueParams { until: Some(c.to.clone()), ..Default::default() },
    )?
    .into_iter()
    .map(|i| {
        vec![
            code(i.patient_number),
            text(i.patient_name),
            opt_code(i.phone),
            code(i.invoice_number),
            code(format!("{}/{}", i.no, i.count)),
            date(i.due_date),
            money(i.amount),
            money(i.paid),
            money(i.amount - i.paid),
            int(i.days_late),
            tag(match i.state {
                InstallmentState::Overdue => "overdue",
                InstallmentState::Partial => "partial",
                InstallmentState::Paid => "paid",
                InstallmentState::Pending => "pending",
            }),
        ]
    })
    .collect();
    Ok(table(
        &[
            "patient_number",
            "patient",
            "phone",
            "invoice",
            "installment",
            "due",
            "amount",
            "paid",
            "unpaid",
            "days_late",
            "state",
        ],
        rows,
        &[6, 7, 8],
    ))
}

fn deposits(c: &Ctx) -> Result<Table> {
    let (_, to) = days(c);
    let mut stmt = c.conn.prepare(&format!(
        "SELECT p.patient_number, p.full_name, -SUM(g.amount) FROM fin_gl g JOIN patient p ON p.id = g.party_id
         WHERE g.account_id = 2210 AND g.date <= ?1 GROUP BY g.party_id HAVING SUM(g.amount) <> 0
         ORDER BY 3 DESC {CAP}"
    ))?;
    let rows = stmt
        .query_map([to], |r| {
            Ok(vec![code(r.get::<_, String>(0)?), text(r.get::<_, String>(1)?), money(r.get(2)?)])
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(table(&["patient_number", "patient", "deposit"], rows, &[2]))
}

fn expenses(c: &Ctx) -> Result<Table> {
    let (from, to) = days(c);
    let mut stmt = c.conn.prepare(&format!(
        "SELECT d.doc_date, {NUMBER_SQL}, d.kind, l.account_id, COALESCE(l.name, d.note, ''), COALESCE(s.name, d.name),
            s.tin, l.amount, COALESCE(l.withheld, 0), d.account_id, d.status
         FROM fin_line l JOIN fin_doc d ON d.id = l.doc_id JOIN fin_account a ON a.id = l.account_id
            LEFT JOIN supplier s ON s.id = d.party_id
         WHERE d.doc_date BETWEEN ?1 AND ?2 AND a.type IN ('expense', 'asset') AND a.money IS NULL
            AND (d.kind = 'bl' OR (d.kind = 'pv' AND d.purpose IN ('expense', 'other')))
         ORDER BY d.doc_date, d.id, l.no {CAP}"
    ))?;
    type R =
        (i64, String, String, i64, String, Option<String>, Option<String>, i64, i64, Option<i64>, String);
    let raw: Vec<R> = stmt
        .query_map(params![from, to], |r| {
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
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut rows = Vec::with_capacity(raw.len());
    for (d, number, kind, account, desc, payee, tin, amount, withheld, paid_from, status) in raw {
        let posted = status == "posted";
        let z = |v: i64| money(if posted { v } else { 0 });
        rows.push(vec![
            date(iso_of(d)),
            code(number),
            tag(if kind == "bl" { "bill" } else { "paid" }),
            account_cell(c.conn, Some(account))?,
            opt_text(Some(desc)),
            opt_text(payee),
            opt_code(tin),
            z(amount),
            z(withheld),
            z(amount - withheld),
            account_cell(c.conn, paid_from)?,
            status_tag(&status),
        ]);
    }
    Ok(table(
        &[
            "date",
            "number",
            "kind",
            "account",
            "description",
            "payee",
            "tin",
            "gross",
            "withheld",
            "net",
            "paid_from",
            "status",
        ],
        rows,
        &[7, 8, 9],
    ))
}

fn money_book(c: &Ctx) -> Result<Table> {
    let account = c.account.unwrap_or(1110);
    let l = fin::accounts::ledger(
        c.conn,
        &AccountLedgerParams { account_id: account, party_id: None, from: c.from.clone(), to: c.to.clone() },
    )?;
    let mut rows = vec![vec![
        date(l.from.clone()),
        ReportCell::Empty,
        tag("opening"),
        ReportCell::Empty,
        money(0),
        money(0),
        money(l.opening),
    ]];
    for x in l.lines {
        rows.push(vec![
            date(x.date),
            code(x.number),
            tag(if x.reversal { "void".to_string() } else { format!("doc_{}", fin_kind_code(x.kind)) }),
            opt_text(Some(x.description)),
            money(x.amount.max(0)),
            money((-x.amount).max(0)),
            money(x.balance),
        ]);
    }
    Ok(table(&["date", "number", "kind", "description", "money_in", "money_out", "balance"], rows, &[4, 5]))
}

fn fin_kind_code(k: DocKind) -> &'static str {
    fin::ledger::kind_code(k)
}

/// FR-05: a line's share of the price reductions (CN) made on its invoice later, in proportion to the
/// line's net. Needs `fin_doc d` (the invoice) and `fin_line l`.
const REDUCTION_SHARE: &str = "(CASE WHEN d.amount > 0 THEN (l.amount - COALESCE(l.discount, 0))
        * (SELECT COALESCE(SUM(x.amount), 0) FROM fin_doc x WHERE x.ref_id = d.id AND x.kind = 'cn' AND x.status = 'posted')
        / d.amount ELSE 0 END)";

fn provider_production(c: &Ctx) -> Result<Table> {
    let (from, to) = days(c);
    let mut stmt = c.conn.prepare(&format!(
        "SELECT COALESCE(dr.full_name, ''), COUNT(DISTINCT d.party_id), COALESCE(SUM(l.qty), 0), COALESCE(SUM(l.amount), 0),
            COALESCE(SUM(COALESCE(l.discount, 0) + {REDUCTION_SHARE}), 0)
         FROM fin_line l JOIN fin_doc d ON d.id = l.doc_id LEFT JOIN doctor dr ON dr.id = l.provider_id
         WHERE d.kind = 'inv' AND d.status = 'posted' AND d.doc_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR l.provider_id = ?3)
         GROUP BY l.provider_id ORDER BY 4 DESC"
    ))?;
    let rows = stmt
        .query_map(params![from, to, c.doctor], |r| {
            let (gross, discount): (i64, i64) = (r.get(3)?, r.get(4)?);
            Ok(vec![
                opt_text(r.get(0)?),
                int(r.get(1)?),
                int(r.get(2)?),
                money(gross),
                money(discount),
                money(gross - discount),
            ])
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(table(&["doctor", "patients", "units", "gross", "discount", "net"], rows, &[2, 3, 4, 5]))
}

fn service_revenue(c: &Ctx) -> Result<Table> {
    let (from, to) = days(c);
    let mut stmt = c.conn.prepare(&format!(
        "SELECT s.name_fa, s.name_ps, s.name_en, COALESCE(SUM(l.qty), 0),
            COALESCE(SUM(l.amount - COALESCE(l.discount, 0) - {REDUCTION_SHARE}), 0)
         FROM fin_line l JOIN fin_doc d ON d.id = l.doc_id JOIN service s ON s.id = l.service_id
         WHERE d.kind = 'inv' AND d.status = 'posted' AND d.doc_date BETWEEN ?1 AND ?2
           AND (?3 IS NULL OR l.service_id = ?3) AND (?4 IS NULL OR l.provider_id = ?4)
         GROUP BY s.id ORDER BY 5 DESC"
    ))?;
    let rows = stmt
        .query_map(params![from, to, c.service, c.doctor], |r| {
            Ok(vec![name(r.get(0)?, r.get(1)?, r.get(2)?), int(r.get(3)?), money(r.get(4)?)])
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(table(&["service", "units", "net"], rows, &[1, 2]))
}

fn doctor_shares(c: &Ctx) -> Result<Table> {
    let report = fin::shares::preview(
        c.conn,
        &CommissionReportParams { doctor_id: c.doctor.clone(), from: c.from.clone(), to: c.to.clone() },
    )?;
    let rows = report
        .doctors
        .into_iter()
        .map(|d| vec![text(d.doctor_name), int(d.lines.len() as i64), money(d.base), money(d.commission)])
        .collect();
    Ok(table(&["doctor", "lines", "base", "share"], rows, &[1, 2, 3]))
}

fn cash_counts(c: &Ctx) -> Result<Table> {
    let list = fin::cash::counts(
        c.conn,
        &CashCountListParams { account_id: c.account, from: Some(c.from.clone()), to: Some(c.to.clone()) },
    )?;
    let mut rows = Vec::with_capacity(list.len());
    for k in list.into_iter().rev() {
        rows.push(vec![
            date(k.date),
            code(k.number),
            account_cell(c.conn, Some(k.account_id))?,
            money(k.book),
            money(k.counted),
            money(k.difference),
            opt_text(k.reason),
            opt_text(k.created_by_name),
        ]);
    }
    Ok(table(
        &["date", "number", "account", "book", "counted", "difference", "reason", "user"],
        rows,
        &[3, 4, 5],
    ))
}

/// Month by month: money in and out (cash method) next to work invoiced and costs incurred (accrual).
fn income_statement(c: &Ctx) -> Result<Table> {
    let (from, to) = days(c);
    let mut stmt = c.conn.prepare(
        "SELECT g.date, CASE WHEN d.kind = 'pv' THEN 'pv:' || d.purpose ELSE d.kind END, g.account_id, a.type,
            a.money IS NOT NULL, g.amount
         FROM fin_gl g JOIN fin_doc d ON d.id = g.doc_id JOIN fin_account a ON a.id = g.account_id
         WHERE g.date BETWEEN ?1 AND ?2",
    )?;
    type R = (i64, String, i64, String, bool, i64);
    let raw: Vec<R> = stmt
        .query_map(params![from, to], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    // (received, cash expenses, revenue, accrual costs)
    let mut months: BTreeMap<String, (i64, i64, i64, i64)> = BTreeMap::new();
    for (day, kind, account, kind_of_account, is_money, amount) in raw {
        let e = months.entry(month_key(&iso_of(day), c.calendar)).or_default();
        if is_money && (kind == "rc" || kind == "rf") {
            e.0 += amount;
        }
        if kind.starts_with("pv") && (kind_of_account == "expense" || account == 2110) {
            e.1 += amount;
        }
        if matches!(account, 4110 | 4190 | 4910 | 4920) {
            e.2 -= amount;
        }
        if kind_of_account == "expense" {
            e.3 += amount;
        }
    }
    let rows = months
        .into_iter()
        .map(|(k, (received, paid, revenue, costs))| {
            vec![
                ReportCell::Month { value: k },
                money(received),
                money(paid),
                money(received - paid),
                money(revenue),
                money(costs),
                money(revenue - costs),
            ]
        })
        .collect();
    Ok(table(
        &["month", "net_received", "paid_out", "cash_result", "revenue", "costs", "accrual_result"],
        rows,
        &[1, 2, 3, 4, 5, 6],
    ))
}

/// R8: every employee line of the payrolls approved in the period.
fn payroll(c: &Ctx) -> Result<Table> {
    let (from, to) = days(c);
    let rows = collect(
        c,
        &format!(
            "SELECT d.doc_date, {NUMBER_SQL}, d.ext_id, e.full_name, e.father_name, e.tin, l.base, l.extra, l.commission,
                l.advance, l.tax
             FROM fin_pay_line l JOIN fin_doc d ON d.id = l.doc_id LEFT JOIN fin_employee e ON e.id = l.employee_id
             WHERE d.kind = 'pr' AND d.status = 'posted' AND d.doc_date BETWEEN ?1 AND ?2
             ORDER BY d.doc_date, d.id, e.full_name {CAP}"
        ),
        &[&from, &to],
        |r| {
            let (base, extra, commission, advance, tax): (i64, i64, i64, i64, i64) =
                (r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?);
            let gross = base + extra + commission;
            let period: Option<String> = r.get(2)?;
            let (f, t) = fin::shares::period_of(&period);
            Ok(vec![
                date(iso_of(r.get(0)?)),
                code(r.get::<_, String>(1)?),
                text(format!("{} – {}", iso_of(f), iso_of(t))),
                opt_text(r.get(3)?),
                opt_text(r.get(4)?),
                opt_code(r.get(5)?),
                money(base),
                money(extra),
                money(commission),
                money(gross),
                money(advance),
                money(tax),
                money(gross - advance - tax),
            ])
        },
    )?;
    Ok(table(
        &[
            "date",
            "number",
            "period",
            "employee",
            "father_name",
            "tin",
            "base",
            "extra",
            "commission",
            "gross",
            "advance",
            "tax",
            "net",
        ],
        rows,
        &[6, 7, 8, 9, 10, 11, 12],
    ))
}

/// R9: tax withheld (salaries, rent, contractors) and paid to the government, month by month, with
/// what is still to pay at the end of each month.
fn withholding(c: &Ctx) -> Result<Table> {
    let (from, to) = days(c);
    let mut stmt = c.conn.prepare(
        "SELECT g.date, g.account_id, (d.kind = 'pv' AND d.purpose = 'tax'), g.amount
         FROM fin_gl g JOIN fin_doc d ON d.id = g.doc_id
         WHERE g.account_id IN (2410, 2420, 2430) AND g.date BETWEEN ?1 AND ?2 ORDER BY g.date",
    )?;
    let raw: Vec<(i64, i64, bool, i64)> = stmt
        .query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    // (account, month) -> (withheld, paid)
    let mut months: BTreeMap<(i64, String), (i64, i64)> = BTreeMap::new();
    for (day, account, paid, amount) in raw {
        let e = months.entry((account, month_key(&iso_of(day), c.calendar))).or_default();
        if paid {
            e.1 += amount;
        } else {
            e.0 -= amount;
        }
    }
    let mut rows = Vec::new();
    let mut open: BTreeMap<i64, i64> = BTreeMap::new();
    for ((account, month), (withheld, paid)) in months {
        let left = match open.entry(account) {
            std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert(-fin::ledger::balance(c.conn, account, None, from - 1)?)
            }
        };
        *left += withheld - paid;
        rows.push(vec![
            ReportCell::Month { value: month },
            tag(format!("acct_{account}")),
            money(withheld),
            money(paid),
            money(*left),
        ]);
    }
    Ok(table(&["month", "account", "withheld", "paid", "unpaid"], rows, &[2, 3]))
}

fn patients(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        &format!(
            "SELECT p.registration_date, p.patient_number, p.full_name, p.phone,
                    COALESCE((SELECT label FROM reference_translation WHERE reference_item_id = p.referral_source_id AND language_code = 'fa'), ''),
                    COALESCE((SELECT label FROM reference_translation WHERE reference_item_id = p.referral_source_id AND language_code = 'ps'), ''),
                    COALESCE((SELECT label FROM reference_translation WHERE reference_item_id = p.referral_source_id AND language_code = 'en'), '')
             FROM patient p WHERE p.deleted_at IS NULL AND p.registration_date BETWEEN ?1 AND ?2
             ORDER BY p.registration_date, p.patient_number {CAP}"
        ),
        &[&c.from, &c.to],
        |r| {
            let (fa, ps, en): (String, String, String) = (r.get(4)?, r.get(5)?, r.get(6)?);
            let source = if fa.is_empty() && ps.is_empty() && en.is_empty() { ReportCell::Empty } else { name(fa, ps, en) };
            Ok(vec![date(r.get::<_, String>(0)?), code(r.get::<_, String>(1)?), text(r.get::<_, String>(2)?), opt_code(r.get(3)?), source])
        },
    )?;
    let n = rows.len() as i64;
    let mut t = table(&["registered", "number", "patient", "phone", "source"], rows, &[]);
    if n > 0 {
        t.totals = vec![int(n), ReportCell::Empty, ReportCell::Empty, ReportCell::Empty, ReportCell::Empty];
    }
    Ok(t)
}

fn treatments(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        &format!(
            "SELECT t.completed_at, pt.full_name, s.name_fa, s.name_ps, s.name_en, t.teeth, COALESCE(dr.full_name, ''), t.units, t.unit_price * t.units
             FROM treatment_item t JOIN patient pt ON pt.id = t.patient_id JOIN service s ON s.id = t.service_id
                  LEFT JOIN doctor dr ON dr.id = t.doctor_id
             WHERE t.deleted_at IS NULL AND t.status = 'completed' AND t.completed_at >= ?1 AND t.completed_at < ?2
               AND (?3 IS NULL OR t.doctor_id = ?3) AND (?4 IS NULL OR t.service_id = ?4)
             ORDER BY t.completed_at {CAP}"
        ),
        &[&c.lo, &c.hi, &c.doctor, &c.service],
        |r| {
            let at: String = r.get(0)?;
            Ok(vec![
                // The stored instant is UTC; the clinic's day starts 4½ hours earlier, so shift before cutting the date.
                date(local_day(&at)),
                text(r.get::<_, String>(1)?),
                name(r.get(2)?, r.get(3)?, r.get(4)?),
                opt_code(r.get(5)?),
                opt_text(r.get(6)?),
                int(r.get(7)?),
                money(r.get(8)?),
            ])
        },
    )?;
    Ok(table(&["date", "patient", "service", "teeth", "doctor", "units", "value"], rows, &[5, 6]))
}

/// The clinic-local day of a stored UTC instant.
fn local_day(ts: &str) -> String {
    parse(ts).map(|t| format_date(t.to_offset(CLINIC_OFFSET).date())).unwrap_or_else(|| day_of(ts))
}

fn procedures(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        "SELECT s.name_fa, s.name_ps, s.name_en, COALESCE(SUM(t.units), 0), COUNT(DISTINCT t.patient_id), COALESCE(SUM(t.unit_price * t.units), 0)
         FROM treatment_item t JOIN service s ON s.id = t.service_id
         WHERE t.deleted_at IS NULL AND t.status = 'completed' AND t.completed_at >= ?1 AND t.completed_at < ?2
           AND (?3 IS NULL OR t.doctor_id = ?3) AND (?4 IS NULL OR t.service_id = ?4)
         GROUP BY s.id ORDER BY 4 DESC",
        &[&c.lo, &c.hi, &c.doctor, &c.service],
        |r| Ok(vec![name(r.get(0)?, r.get(1)?, r.get(2)?), int(r.get(3)?), int(r.get(4)?), money(r.get(5)?)]),
    )?;
    Ok(table(&["service", "units", "patients", "value"], rows, &[1, 3]))
}

fn tooth_conditions(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        "SELECT k.name_fa, k.name_ps, k.name_en, COUNT(*), COUNT(DISTINCT f.patient_id)
         FROM tooth_finding f JOIN tooth_condition k ON k.id = f.condition_id
         WHERE f.deleted_at IS NULL AND f.recorded_on BETWEEN ?1 AND ?2
         GROUP BY k.id ORDER BY 4 DESC",
        &[&c.from, &c.to],
        |r| Ok(vec![name(r.get(0)?, r.get(1)?, r.get(2)?), int(r.get(3)?), int(r.get(4)?)]),
    )?;
    Ok(table(&["condition", "findings", "patients"], rows, &[1]))
}

fn doctor_activity(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        "SELECT d.full_name,
            (SELECT COUNT(*) FROM appointment a WHERE a.doctor_id = d.id AND a.deleted_at IS NULL AND a.local_date BETWEEN ?1 AND ?2),
            (SELECT COUNT(*) FROM appointment a WHERE a.doctor_id = d.id AND a.deleted_at IS NULL AND a.local_date BETWEEN ?1 AND ?2 AND a.status = 'completed'),
            (SELECT COUNT(*) FROM appointment a WHERE a.doctor_id = d.id AND a.deleted_at IS NULL AND a.local_date BETWEEN ?1 AND ?2 AND a.status = 'no_show'),
            (SELECT COUNT(*) FROM appointment a WHERE a.doctor_id = d.id AND a.deleted_at IS NULL AND a.local_date BETWEEN ?1 AND ?2 AND a.status = 'cancelled'),
            (SELECT COUNT(*) FROM treatment_item t WHERE t.doctor_id = d.id AND t.deleted_at IS NULL AND t.status = 'completed'
                AND t.completed_at >= ?3 AND t.completed_at < ?4)
         FROM doctor d WHERE d.deleted_at IS NULL AND (?5 IS NULL OR d.id = ?5) ORDER BY d.sort_order, d.full_name",
        &[&c.from, &c.to, &c.lo, &c.hi, &c.doctor],
        |r| Ok(vec![text(r.get::<_, String>(0)?), int(r.get(1)?), int(r.get(2)?), int(r.get(3)?), int(r.get(4)?), int(r.get(5)?)]),
    )?;
    Ok(table(
        &["doctor", "visits", "completed", "no_show", "cancelled", "treatments"],
        rows,
        &[1, 2, 3, 4, 5],
    ))
}

fn recall_rows(c: &Ctx, only_follow_up: bool) -> Result<Rows> {
    let kinds = if only_follow_up {
        "r.kind IN ('follow_up', 'no_show')"
    } else {
        "r.status IN ('pending', 'contacted')"
    };
    let range =
        if only_follow_up { "r.due_date BETWEEN ?1 AND ?2" } else { "r.due_date <= ?2 AND ?1 IS NOT NULL" };
    collect(
        c,
        &format!(
            "SELECT r.due_date, pt.full_name, pt.phone, r.kind, r.status, r.last_contacted_at, r.note
             FROM recall r JOIN patient pt ON pt.id = r.patient_id
             WHERE r.deleted_at IS NULL AND pt.deleted_at IS NULL AND {kinds} AND {range}
             ORDER BY r.due_date, pt.full_name {CAP}"
        ),
        &[&c.from, &c.to],
        |r| {
            Ok(vec![
                date(r.get::<_, String>(0)?),
                text(r.get::<_, String>(1)?),
                opt_code(r.get(2)?),
                tag(r.get::<_, String>(3)?),
                tag(r.get::<_, String>(4)?),
                opt_date(r.get::<_, Option<String>>(5)?.map(|t| local_day(&t))),
                opt_text(r.get(6)?),
            ])
        },
    )
}

fn follow_ups(c: &Ctx) -> Result<Table> {
    Ok(table(&["due", "patient", "phone", "kind", "status", "contacted", "note"], recall_rows(c, true)?, &[]))
}

/// Everyone still waiting to be called back, up to the end of the period (overdue ones included).
fn recall_list(c: &Ctx) -> Result<Table> {
    Ok(table(
        &["due", "patient", "phone", "kind", "status", "contacted", "note"],
        recall_rows(c, false)?,
        &[],
    ))
}

fn referral_sources(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        "SELECT COALESCE((SELECT label FROM reference_translation WHERE reference_item_id = p.referral_source_id AND language_code = 'fa'), ''),
                COALESCE((SELECT label FROM reference_translation WHERE reference_item_id = p.referral_source_id AND language_code = 'ps'), ''),
                COALESCE((SELECT label FROM reference_translation WHERE reference_item_id = p.referral_source_id AND language_code = 'en'), ''),
                COUNT(*)
         FROM patient p WHERE p.deleted_at IS NULL AND p.registration_date BETWEEN ?1 AND ?2
         GROUP BY p.referral_source_id ORDER BY 4 DESC",
        &[&c.from, &c.to],
        |r| {
            let (fa, ps, en): (String, String, String) = (r.get(0)?, r.get(1)?, r.get(2)?);
            let source = if fa.is_empty() && ps.is_empty() && en.is_empty() { tag("unknown") } else { name(fa, ps, en) };
            Ok(vec![source, int(r.get(3)?)])
        },
    )?;
    Ok(table(&["source", "patients"], rows, &[1]))
}

fn stock(c: &Ctx) -> Result<Table> {
    let rows = inventory::list_items(c.conn, &StockItemListParams::default())?
        .into_iter()
        .map(|i| {
            vec![
                opt_code(i.code),
                ReportCell::Name { value: i.name },
                ReportCell::Name { value: i.category },
                ReportCell::Name { value: i.unit },
                qty(i.qty),
                qty(i.usable_qty),
                qty(i.expired_qty),
                money(i.value),
                opt_date(i.nearest_expiry),
            ]
        })
        .collect();
    Ok(table(
        &["code", "item", "category", "unit", "on_hand", "usable", "expired", "value", "nearest_expiry"],
        rows,
        &[7],
    ))
}

fn low_stock(c: &Ctx) -> Result<Table> {
    let rows = inventory::list_items(
        c.conn,
        &StockItemListParams { only: Some(StockFilter::Low), ..Default::default() },
    )?
    .into_iter()
    .map(|i| {
        vec![
            opt_code(i.code),
            ReportCell::Name { value: i.name },
            ReportCell::Name { value: i.unit },
            qty(i.usable_qty),
            qty(i.min_qty),
            qty((i.min_qty - i.usable_qty).max(0)),
        ]
    })
    .collect();
    Ok(table(&["code", "item", "unit", "usable", "minimum", "missing"], rows, &[]))
}

fn purchases(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        &format!(
            "SELECT p.local_date, p.number, s.name, p.total, p.status
             FROM purchase p JOIN supplier s ON s.id = p.supplier_id
             WHERE p.local_date BETWEEN ?1 AND ?2 ORDER BY p.local_date, p.number {CAP}"
        ),
        &[&c.from, &c.to],
        |r| {
            Ok(vec![
                date(r.get::<_, String>(0)?),
                code(r.get::<_, String>(1)?),
                text(r.get::<_, String>(2)?),
                money(r.get(3)?),
                tag(r.get::<_, String>(4)?),
            ])
        },
    )?;
    // A voided purchase is shown but is not money spent.
    let spent: i64 = rows
        .iter()
        .filter(|r| r.get(4) != Some(&tag("void")))
        .filter_map(|r| if let Some(ReportCell::Money { value }) = r.get(3) { Some(*value) } else { None })
        .sum();
    let mut t = table(&["date", "number", "supplier", "total", "status"], rows, &[]);
    if !t.rows.is_empty() {
        t.totals =
            vec![ReportCell::Empty, ReportCell::Empty, ReportCell::Empty, money(spent), ReportCell::Empty];
    }
    Ok(t)
}

fn consumption(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        "SELECT i.name_fa, i.name_ps, i.name_en, u.name_fa, u.name_ps, u.name_en,
                COALESCE(SUM(-m.qty), 0), COALESCE(SUM(-m.qty * m.unit_cost / 100), 0)
         FROM stock_movement m JOIN stock_item i ON i.id = m.item_id JOIN stock_unit u ON u.id = i.unit_id
         WHERE m.kind = 'consume' AND m.local_date BETWEEN ?1 AND ?2
         GROUP BY m.item_id ORDER BY 8 DESC",
        &[&c.from, &c.to],
        |r| {
            Ok(vec![
                name(r.get(0)?, r.get(1)?, r.get(2)?),
                name(r.get(3)?, r.get(4)?, r.get(5)?),
                qty(r.get(6)?),
                money(r.get(7)?),
            ])
        },
    )?;
    Ok(table(&["item", "unit", "consumed", "value"], rows, &[3]))
}

fn expiry(c: &Ctx) -> Result<Table> {
    let days = (parse_date("to", &c.to)? - parse_date("today", &today_iso())?).whole_days().max(60);
    let rows = inventory::list_batches(
        c.conn,
        &StockBatchListParams { expiring_days: Some(days), ..Default::default() },
    )?
    .into_iter()
    .filter(|b| b.state != BatchState::Valid)
    .map(|b| {
        vec![
            ReportCell::Name { value: b.item_name },
            opt_code(b.lot_no),
            opt_date(b.expiry_date),
            qty(b.qty_left),
            ReportCell::Name { value: b.location },
            tag(if b.state == BatchState::Expired { "expired" } else { "expiring" }),
            money(b.qty_left * b.unit_cost / 100),
        ]
    })
    .collect();
    Ok(table(&["item", "lot", "expiry", "qty_left", "location", "status", "value"], rows, &[6]))
}

fn supplier_balance(c: &Ctx) -> Result<Table> {
    let rows = inventory::list_suppliers(
        c.conn,
        &SupplierListParams { kind: None, include_inactive: false, owing_only: false },
    )?
    .into_iter()
    .map(|s| {
        vec![
            text(s.name),
            tag(if s.kind == SupplierKind::Lab { "lab" } else { "supplier" }),
            money(s.purchases_total),
            money(s.lab_total),
            money(s.returns_total),
            money(s.paid_total),
            money(s.balance),
        ]
    })
    .collect();
    Ok(table(&["supplier", "kind", "purchases", "lab", "returns", "paid", "balance"], rows, &[2, 3, 4, 5, 6]))
}

// ───────────────────────────── Excel export ─────────────────────────────

fn xlsx_error(e: rust_xlsxwriter::XlsxError) -> CoreError {
    CoreError::api(ErrorCode::Internal, format!("cannot write the Excel file: {e}"))
}

fn pick(t: &Translations, language: Language) -> String {
    let s = match language {
        Language::Fa => &t.fa,
        Language::Ps => &t.ps,
        Language::En => &t.en,
    };
    if s.is_empty() {
        t.fa.clone()
    } else {
        s.clone()
    }
}

pub fn export(
    conn: &Connection,
    actor: &Actor,
    p: &ExportReportParams,
    exports_dir: &Path,
) -> Result<ExportResult> {
    use rust_xlsxwriter::{Format, Workbook};
    let result = run(conn, &p.report)?;
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    let title: String = p.title.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
    sheet.set_name(if title.trim().is_empty() { "Report" } else { title.trim() }).map_err(xlsx_error)?;
    sheet.set_right_to_left(matches!(p.language, Language::Fa | Language::Ps));
    let bold = Format::new().set_bold();
    let percent = Format::new().set_num_format("0.00%");
    let amount = Format::new().set_num_format("#,##0.00");
    let total_amount = Format::new().set_bold().set_num_format("#,##0.00");
    let label = |v: &str| p.labels.get(v).cloned().unwrap_or_else(|| v.to_string());
    for (i, h) in p.headers.iter().enumerate().take(result.columns.len()) {
        sheet.write_string_with_format(0, i as u16, h, &bold).map_err(xlsx_error)?;
        sheet.set_column_width(i as u16, 20.0).map_err(xlsx_error)?;
    }
    let write = |sheet: &mut rust_xlsxwriter::Worksheet,
                 row: u32,
                 col: u16,
                 cell: &ReportCell,
                 strong: bool|
     -> Result<()> {
        let money_fmt = if strong { &total_amount } else { &amount };
        match cell {
            ReportCell::Empty => {}
            ReportCell::Text { value }
            | ReportCell::Code { value }
            | ReportCell::Date { value }
            | ReportCell::Month { value } => {
                sheet.write_string(row, col, value).map_err(xlsx_error)?;
            }
            ReportCell::Tag { value } => {
                sheet.write_string(row, col, label(value)).map_err(xlsx_error)?;
            }
            ReportCell::Name { value } => {
                sheet.write_string(row, col, pick(value, p.language)).map_err(xlsx_error)?;
            }
            ReportCell::Int { value } => {
                sheet.write_number(row, col, *value as f64).map_err(xlsx_error)?;
            }
            ReportCell::Money { value } | ReportCell::Qty { value } => {
                sheet
                    .write_number_with_format(row, col, *value as f64 / 100.0, money_fmt)
                    .map_err(xlsx_error)?;
            }
            ReportCell::Percent { value } => {
                sheet
                    .write_number_with_format(row, col, *value as f64 / 10_000.0, &percent)
                    .map_err(xlsx_error)?;
            }
        }
        Ok(())
    };
    for (r, row) in result.rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            write(sheet, (r + 1) as u32, c as u16, cell, false)?;
        }
    }
    if !result.totals.is_empty() {
        let at = (result.rows.len() + 1) as u32;
        for (c, cell) in result.totals.iter().enumerate() {
            write(sheet, at, c as u16, cell, true)?;
        }
    }
    sheet.set_freeze_panes(1, 0).map_err(xlsx_error)?;
    let bytes = book.save_to_buffer().map_err(xlsx_error)?;

    let stamp: String = now_iso().chars().take(19).map(|c| if c == ':' { '-' } else { c }).collect();
    let kind = serde_json::to_value(p.report.kind)?.as_str().unwrap_or("report").to_string();
    std::fs::create_dir_all(exports_dir)?;
    // FR-09: two exports in the same second must not overwrite each other.
    let mut file_name = format!("report-{kind}-{stamp}.xlsx");
    let mut n = 1;
    while exports_dir.join(&file_name).exists() {
        n += 1;
        file_name = format!("report-{kind}-{stamp}-{n}.xlsx");
    }
    let path = exports_dir.join(&file_name);
    std::fs::write(&path, &bytes)?;
    audit::record(
        conn,
        actor,
        "report.export",
        Some("report"),
        None,
        None,
        Some(
            &json!({ "kind": kind, "from": result.from, "to": result.to, "rows": result.rows.len(), "file": file_name }),
        ),
    )?;
    Ok(ExportResult { file_path: path.display().to_string(), file_name, rows: result.rows.len() as u32 })
}

// ───────────────────────────── notifications (9.6) ─────────────────────────────

fn same(s: &str) -> Translations {
    Translations { fa: s.to_string(), ps: s.to_string(), en: s.to_string() }
}

#[allow(clippy::too_many_arguments)]
fn item(
    key: String,
    kind: NotificationKind,
    level: NotificationLevel,
    subject: Translations,
    detail: Option<String>,
    date: Option<String>,
    page: &str,
    patient_id: Option<String>,
) -> NotificationItem {
    NotificationItem {
        key,
        kind,
        level,
        subject,
        detail,
        date,
        page: page.to_string(),
        patient_id,
        read: false,
    }
}

fn read_keys(conn: &Connection, user_id: &str) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT key FROM notification_read WHERE user_id = ?1")?;
    let keys = stmt
        .query_map([user_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<HashSet<String>>>()?;
    Ok(keys)
}

/// Facts the database does not hold (the license file, the USB drives plugged in right now).
#[derive(Debug, Clone)]
pub struct NotifyExtras {
    pub license: LicenseStatus,
    pub usb_connected: bool,
    pub usb_cloned: bool,
}

/// What needs attention right now for this user. Nothing is stored except which keys were read.
pub fn notifications(conn: &Connection, s: &Session, extras: &NotifyExtras) -> Result<NotificationList> {
    use NotificationKind as K;
    use NotificationLevel as L;
    let can = |p: &str| s.permissions.contains(p);
    let doctor_id: Option<String> = own_doctor(conn, s)?.map(|d| d.0);
    let today = today_iso();
    let mut items: Vec<NotificationItem> = Vec::new();

    if can(perm::APPOINTMENTS_VIEW) {
        let soon = iso(now() + time::Duration::minutes(90));
        let mut stmt = conn.prepare(
            "SELECT a.id, a.patient_id, p.full_name, a.local_date, a.start_min FROM appointment a
             JOIN patient p ON p.id = a.patient_id
             WHERE a.deleted_at IS NULL AND a.start_at >= ?1 AND a.start_at <= ?2 AND a.status IN ('scheduled', 'confirmed')
               AND (?3 IS NULL OR a.doctor_id = ?3) ORDER BY a.start_at LIMIT 30",
        )?;
        let rows: Vec<(String, String, String, String, i64)> = stmt
            .query_map(params![now_iso(), soon, doctor_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        for (id, patient_id, patient, day, start) in rows {
            items.push(item(
                format!("appointment_reminder:{id}"),
                K::AppointmentReminder,
                L::Info,
                same(&patient),
                Some(format_minutes(start)),
                Some(day),
                "appointments",
                Some(patient_id),
            ));
        }
        let mut stmt = conn.prepare(
            "SELECT r.id, r.patient_id, p.full_name, r.due_date FROM recall r JOIN patient p ON p.id = r.patient_id
             WHERE r.deleted_at IS NULL AND p.deleted_at IS NULL AND r.status IN ('pending', 'contacted') AND r.due_date <= ?1
             ORDER BY r.due_date LIMIT 50",
        )?;
        let rows: Vec<(String, String, String, String)> = stmt
            .query_map([&today], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, patient_id, patient, due) in rows {
            let level = if due < today { L::Warning } else { L::Info };
            items.push(item(
                format!("follow_up_due:{id}"),
                K::FollowUpDue,
                level,
                same(&patient),
                None,
                Some(due),
                "recalls",
                Some(patient_id),
            ));
        }
    }

    if can(perm::BILLING_VIEW) && doctor_id.is_none() {
        for d in fin::patients::due(conn, &InstallmentsDueParams::default())?.into_iter().take(50) {
            let level = if d.state == InstallmentState::Overdue { L::Danger } else { L::Warning };
            items.push(item(
                format!("installment_due:{}:{}", d.invoice_id, d.no),
                K::InstallmentDue,
                level,
                same(&d.patient_name),
                Some((d.amount - d.paid).to_string()),
                Some(d.due_date),
                "billing",
                Some(d.patient_id),
            ));
        }
    }

    if can(perm::INVENTORY_VIEW) {
        let alerts = inventory::alerts(conn, &StockAlertParams::default())?;
        if doctor_id.is_none() {
            for i in alerts.low {
                items.push(item(
                    format!("low_stock:{}", i.id),
                    K::LowStock,
                    L::Warning,
                    i.name,
                    None,
                    None,
                    "inventory",
                    None,
                ));
            }
            for b in alerts.expired {
                items.push(item(
                    format!("expired_item:{}", b.id),
                    K::ExpiredItem,
                    L::Danger,
                    b.item_name,
                    b.lot_no,
                    b.expiry_date,
                    "inventory",
                    None,
                ));
            }
            for b in alerts.expiring {
                items.push(item(
                    format!("expiring_item:{}", b.id),
                    K::ExpiringItem,
                    L::Warning,
                    b.item_name,
                    b.lot_no,
                    b.expiry_date,
                    "inventory",
                    None,
                ));
            }
        }
        for l in alerts.lab_delayed.into_iter().filter(|l| doctor_id.is_none() || l.doctor_id == doctor_id) {
            items.push(item(
                format!("lab_delayed:{}", l.id),
                K::LabDelayed,
                L::Warning,
                same(&l.patient_name),
                Some(l.days_late.to_string()),
                l.expected_date,
                "inventory",
                Some(l.patient_id),
            ));
        }
    }

    if can(perm::BACKUP_VIEW) {
        let b = backup_status(conn)?;
        if b.failed_since {
            let last_failed: i64 = scalar(
                conn,
                "SELECT COALESCE(MAX(id), 0) FROM audit_log WHERE action = 'backup.failed'",
                &[],
            )?;
            items.push(item(
                format!("backup_failed:{last_failed}"),
                K::BackupFailed,
                L::Danger,
                same(""),
                None,
                None,
                "backup",
                None,
            ));
        } else if b.age_hours.is_none_or(|h| h >= BACKUP_OVERDUE_HOURS) {
            let detail = b.age_hours.map(|h| h.to_string());
            items.push(item(
                format!("backup_overdue:{today}"),
                K::BackupOverdue,
                L::Warning,
                same(""),
                detail,
                b.last_at.map(|t| local_day(&t)),
                "backup",
                None,
            ));
        }
        // 10.4: the trusted USB drive is not plugged in, so the newest backup waits for it. A new
        // reminder every `usb_warn_minutes` (19:00, 19:30, 20:00, ...), each one a fresh, unread item.
        let waiting: Option<(String, String)> = conn
            .query_row(
                "SELECT l.id, l.created_at FROM backup_copy c JOIN backup_log l ON l.id = c.backup_id
                 WHERE c.target = 'usb' AND c.status IN ('pending', 'failed') AND l.deleted_at IS NULL
                 ORDER BY l.created_at DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let (Some((id, since)), false) = (waiting, extras.usb_connected) {
            let interval = crate::backup::get_settings(conn)?.usb_warn_minutes.max(5) as i64;
            let minutes = parse(&since).map(|t| (now() - t).whole_minutes()).unwrap_or(0).max(0);
            items.push(item(
                format!("usb_missing:{id}:{}", minutes / interval),
                K::UsbMissing,
                L::Warning,
                same(""),
                Some(minutes.to_string()),
                Some(local_day(&since)),
                "backup",
                None,
            ));
        }
        if extras.usb_cloned {
            items.push(item(
                format!("usb_cloned:{today}"),
                K::UsbCloned,
                L::Danger,
                same(""),
                None,
                None,
                "backup",
                None,
            ));
        }
    }

    // The license speaks to whoever can change settings (10.8, 10.9).
    if can(perm::SETTINGS_MANAGE) {
        use artaveo_shared::LicenseState as S;
        let l = &extras.license;
        let page = "license";
        if l.read_only {
            items.push(item(
                format!("license_read_only:{:?}", l.state),
                K::LicenseReadOnly,
                L::Danger,
                same(""),
                None,
                None,
                page,
                None,
            ));
        } else if l.state == S::Grace {
            let left = l.grace_days_left.map(|d| d.to_string());
            let day = l.verified_at.as_deref().map(local_day);
            items.push(item(
                format!("license_verify:{}", day.clone().unwrap_or_default()),
                K::LicenseVerify,
                L::Warning,
                same(""),
                left,
                day,
                page,
                None,
            ));
        } else if l.state == S::ExpiredWarning {
            let over = l.days_left.map(|d| (-d).to_string());
            items.push(item(
                format!("license_expired:{}", over.clone().unwrap_or_default()),
                K::LicenseExpired,
                L::Warning,
                same(""),
                over,
                None,
                page,
                None,
            ));
        } else if let Some(left) = l.days_left.filter(|d| (0..=30).contains(d)) {
            items.push(item(
                format!("license_expiring:{left}"),
                K::LicenseExpiring,
                L::Warning,
                same(""),
                Some(left.to_string()),
                None,
                page,
                None,
            ));
        }
    }

    // Most urgent first; inside one level, the order the checks above produced.
    items.sort_by_key(|i| std::cmp::Reverse(i.level));
    let read = read_keys(conn, &s.user.id)?;
    for i in &mut items {
        i.read = read.contains(&i.key);
    }
    let unread = items.iter().filter(|i| !i.read).count() as i64;
    Ok(NotificationList { items, unread })
}

pub fn mark_read(
    conn: &Connection,
    s: &Session,
    p: &NotificationKeysParams,
    extras: &NotifyExtras,
) -> Result<NotificationList> {
    let keys: Vec<String> = if p.keys.is_empty() {
        notifications(conn, s, extras)?.items.into_iter().map(|i| i.key).collect()
    } else {
        p.keys.iter().take(500).cloned().collect()
    };
    let at = now_iso();
    for k in keys.iter().filter(|k| k.len() <= 200) {
        conn.execute(
            "INSERT INTO notification_read(user_id, key, read_at) VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING",
            params![s.user.id, k, at],
        )?;
    }
    // A cause that is gone for three months will not come back under the same key.
    conn.execute(
        "DELETE FROM notification_read WHERE read_at < ?1",
        [iso(now() - time::Duration::days(90))],
    )?;
    notifications(conn, s, extras)
}

// ───────────────────────────── global search (9.7) ─────────────────────────────

/// Which kinds of record this user may find.
#[derive(Debug, Clone, Copy)]
pub struct SearchAccess {
    pub patients: bool,
    pub appointments: bool,
    pub billing: bool,
    pub labs: bool,
}

fn like_end(q: &str) -> String {
    format!("{q}\u{10ffff}")
}

pub fn search(conn: &Connection, p: &SearchParams, access: SearchAccess) -> Result<SearchResults> {
    let started = Instant::now();
    let q = p.query.trim();
    let per = p.per_kind.unwrap_or(5).clamp(1, 20) as usize;
    let mut hits: Vec<SearchHit> = Vec::new();
    if q.chars().count() < 2 {
        return Ok(SearchResults { hits, millis: 0 });
    }
    // Patients through the full-text index; the other records are found by their own number or through these patients.
    let found = patient::list_patients(
        conn,
        &PatientListParams { query: Some(q.to_string()), status: None, limit: 10, offset: 0 },
    )?;
    if access.patients {
        for pt in found.items.iter().take(per) {
            hits.push(SearchHit {
                kind: SearchKind::Patient,
                id: pt.id.clone(),
                patient_id: Some(pt.id.clone()),
                title: pt.full_name.clone(),
                subtitle: Some(match &pt.phone {
                    Some(ph) if !ph.is_empty() => format!("{} · {}", pt.patient_number, ph),
                    _ => pt.patient_number.clone(),
                }),
                date: None,
                page: "patients".into(),
            });
        }
    }
    let ids: Vec<Value> = found.items.iter().map(|x| Value::Text(x.id.clone())).collect();
    let marks = vec!["?"; ids.len()].join(",");
    let number_lo = q.to_uppercase();
    let number_hi = like_end(&number_lo);
    let with_ids = |mut args: Vec<Value>| {
        args.extend(ids.iter().cloned());
        args.push(Value::Integer(per as i64));
        args
    };

    if access.appointments && !ids.is_empty() {
        let mut stmt = conn.prepare(&format!(
            "SELECT a.id, a.patient_id, p.full_name, a.local_date, a.start_min, d.full_name
             FROM appointment a JOIN patient p ON p.id = a.patient_id JOIN doctor d ON d.id = a.doctor_id
             WHERE a.deleted_at IS NULL AND a.patient_id IN ({marks}) ORDER BY a.start_at DESC LIMIT ?"
        ))?;
        let args = with_ids(Vec::new());
        let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
            Ok(SearchHit {
                kind: SearchKind::Appointment,
                id: r.get(0)?,
                patient_id: Some(r.get(1)?),
                title: r.get(2)?,
                subtitle: Some(format!("{} · {}", format_minutes(r.get(4)?), r.get::<_, String>(5)?)),
                date: Some(r.get(3)?),
                page: "appointments".into(),
            })
        })?;
        for h in rows {
            hits.push(h?);
        }
    }
    if access.billing {
        // A finance number (`INV-1405-000012`, `RC-1405-…`) or the invoices and receipts of the patients found.
        let mut stmt = conn.prepare(&format!(
            "SELECT d.id, d.kind, d.party_id, {NUMBER_SQL} AS no, p.full_name, d.doc_date
             FROM fin_doc d JOIN patient p ON p.id = d.party_id
             WHERE d.kind IN ('inv', 'rc') AND d.status = 'posted'
               AND ((no >= ? AND no < ?) OR d.party_id IN ({marks}))
             ORDER BY d.id DESC LIMIT ?"
        ))?;
        let mut args = with_ids(vec![Value::Text(number_lo.clone()), Value::Text(number_hi.clone())]);
        if let Some(Value::Integer(n)) = args.last_mut() {
            *n *= 2;
        }
        let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
            let kind: String = r.get(1)?;
            Ok(SearchHit {
                kind: if kind == "inv" { SearchKind::Invoice } else { SearchKind::Receipt },
                id: r.get::<_, i64>(0)?.to_string(),
                patient_id: r.get(2)?,
                title: r.get(3)?,
                subtitle: Some(r.get(4)?),
                date: Some(iso_of(r.get(5)?)),
                page: "billing".into(),
            })
        })?;
        for h in rows {
            hits.push(h?);
        }
    }
    if access.labs {
        let mut stmt = conn.prepare(&format!(
            "SELECT l.id, l.patient_id, l.number, p.full_name, l.work_type, COALESCE(l.sent_date, substr(l.created_at, 1, 10))
             FROM lab_case l JOIN patient p ON p.id = l.patient_id
             WHERE (l.number >= ? AND l.number < ?) OR l.patient_id IN ({marks})
             ORDER BY l.created_at DESC LIMIT ?"
        ))?;
        let args = with_ids(vec![Value::Text(number_lo), Value::Text(number_hi)]);
        let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
            Ok(SearchHit {
                kind: SearchKind::LabCase,
                id: r.get(0)?,
                patient_id: Some(r.get(1)?),
                title: r.get(2)?,
                subtitle: Some(format!("{} · {}", r.get::<_, String>(3)?, r.get::<_, String>(4)?)),
                date: Some(r.get(5)?),
                page: "inventory".into(),
            })
        })?;
        for h in rows {
            hits.push(h?);
        }
    }
    Ok(SearchResults { hits, millis: started.elapsed().as_millis().min(u32::MAX as u128) as u32 })
}
