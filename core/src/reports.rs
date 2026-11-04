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
use crate::billing::{self, date_of, scalar, trio};
use crate::calendar::GregorianDate;
use crate::clock::{iso, now, now_iso, parse, today_iso, CLINIC_OFFSET};
use crate::error::{CoreError, Result};
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

pub fn dashboard(conn: &Connection, s: &Session) -> Result<Dashboard> {
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
            "SELECT COALESCE(SUM(l.line_total), 0) FROM invoice_line l JOIN invoice i ON i.id = l.invoice_id
             WHERE i.status = 'issued' AND i.deleted_at IS NULL AND i.local_date = ?1 AND l.doctor_id = ?2",
            &[&today, id],
        )?;
        (RevenueBasis::Invoiced, Some(own), None)
    } else {
        let received = scalar(
            conn,
            "SELECT COALESCE(SUM(CASE WHEN kind = 'payment' THEN amount
                                      WHEN kind = 'refund' AND returns_to_credit = 0 THEN -amount ELSE 0 END), 0)
             FROM payment WHERE local_date = ?1",
            &[&today],
        )?;
        let owed: i64 = billing::debtors(conn, &DebtorListParams { search: None, limit: Some(1000) })?
            .iter()
            .map(|d| d.remaining)
            .sum();
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
        license: None,
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
        calendar: p.calendar.unwrap_or_default(),
    };
    use ReportKind::*;
    let mut t = match p.kind {
        DailyRevenue => daily_revenue(&c)?,
        MonthlyRevenue => monthly_revenue(&c)?,
        Payments => payments(&c)?,
        Outstanding => outstanding(&c)?,
        Discounts => discounts(&c)?,
        Refunds => refunds(&c)?,
        DoctorRevenue => doctor_revenue(&c)?,
        DoctorCommission => doctor_commission(&c)?,
        ServiceRevenue => service_revenue(&c)?,
        Expenses => expenses(&c)?,
        ProfitLoss => profit_loss(&c)?,
        CashClosings => cash_closings(&c)?,
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

/// What came in, went out and was invoiced on each day of the period.
#[derive(Default, Clone, Copy)]
struct DayMoney {
    received: i64,
    refunded: i64,
    payments: i64,
    invoiced: i64,
}

fn day_money(c: &Ctx) -> Result<BTreeMap<String, DayMoney>> {
    let mut days: BTreeMap<String, DayMoney> = BTreeMap::new();
    let mut stmt = c.conn.prepare(
        "SELECT local_date,
                COALESCE(SUM(CASE WHEN kind = 'payment' THEN amount ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN kind = 'refund' AND returns_to_credit = 0 THEN amount ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN kind = 'payment' THEN 1 ELSE 0 END), 0)
         FROM payment WHERE local_date BETWEEN ?1 AND ?2 AND kind IN ('payment', 'refund') GROUP BY local_date",
    )?;
    for row in stmt.query_map([&c.from, &c.to], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
    })? {
        let (d, received, refunded, payments) = row?;
        let e = days.entry(d).or_default();
        e.received = received;
        e.refunded = refunded;
        e.payments = payments;
    }
    let mut stmt = c.conn.prepare(
        "SELECT i.local_date,
                COALESCE(SUM((SELECT COALESCE(SUM(line_total), 0) FROM invoice_line WHERE invoice_id = i.id) - i.discount
                    - (SELECT COALESCE(SUM(amount), 0) FROM credit_note WHERE invoice_id = i.id)), 0)
         FROM invoice i WHERE i.status = 'issued' AND i.deleted_at IS NULL AND i.local_date BETWEEN ?1 AND ?2
         GROUP BY i.local_date",
    )?;
    for row in stmt.query_map([&c.from, &c.to], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        let (d, invoiced) = row?;
        days.entry(d).or_default().invoiced = invoiced;
    }
    Ok(days)
}

fn daily_revenue(c: &Ctx) -> Result<Table> {
    let rows = day_money(c)?
        .into_iter()
        .map(|(d, m)| {
            vec![
                date(d),
                int(m.payments),
                money(m.invoiced),
                money(m.received),
                money(m.refunded),
                money(m.received - m.refunded),
            ]
        })
        .collect();
    Ok(table(&["date", "payments", "invoiced", "received", "refunded", "net"], rows, &[1, 2, 3, 4, 5]))
}

fn monthly_revenue(c: &Ctx) -> Result<Table> {
    let mut months: BTreeMap<String, (i64, i64, i64, i64)> = BTreeMap::new();
    for (d, m) in day_money(c)? {
        let e = months.entry(month_key(&d, c.calendar)).or_default();
        e.0 += m.invoiced;
        e.1 += m.received;
        e.2 += m.refunded;
        e.3 += m.payments;
    }
    let rows = months
        .into_iter()
        .map(|(k, (invoiced, received, refunded, n))| {
            vec![
                ReportCell::Month { value: k },
                int(n),
                money(invoiced),
                money(received),
                money(refunded),
                money(received - refunded),
            ]
        })
        .collect();
    Ok(table(&["month", "payments", "invoiced", "received", "refunded", "net"], rows, &[1, 2, 3, 4, 5]))
}

fn payments(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        &format!(
            "SELECT p.local_date, p.number, pt.full_name, COALESCE(m.name_fa, ''), COALESCE(m.name_ps, ''), COALESCE(m.name_en, ''), p.amount
             FROM payment p JOIN patient pt ON pt.id = p.patient_id LEFT JOIN payment_method m ON m.id = p.method_id
             WHERE p.kind = 'payment' AND p.local_date BETWEEN ?1 AND ?2 ORDER BY p.local_date, p.number {CAP}"
        ),
        &[&c.from, &c.to],
        |r| {
            Ok(vec![date(r.get::<_, String>(0)?), code(r.get::<_, String>(1)?), text(r.get::<_, String>(2)?), name(r.get(3)?, r.get(4)?, r.get(5)?), money(r.get(6)?)])
        },
    )?;
    Ok(table(&["date", "number", "patient", "method", "amount"], rows, &[4]))
}

fn outstanding(c: &Ctx) -> Result<Table> {
    let rows = billing::debtors(c.conn, &DebtorListParams { search: None, limit: Some(1000) })?
        .into_iter()
        .map(|d| {
            vec![
                code(d.patient_number),
                text(d.patient_name),
                opt_code(d.phone),
                int(d.invoice_count),
                opt_date(d.oldest_invoice_at.map(|t| day_of(&t))),
                opt_date(d.next_due),
                money(d.remaining),
            ]
        })
        .collect();
    Ok(table(&["number", "patient", "phone", "invoices", "oldest", "next_due", "remaining"], rows, &[3, 6]))
}

fn discounts(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        &format!(
            "SELECT i.local_date, i.number, pt.full_name,
                    (SELECT COALESCE(SUM(line_total), 0) FROM invoice_line WHERE invoice_id = i.id), i.discount
             FROM invoice i JOIN patient pt ON pt.id = i.patient_id
             WHERE i.status = 'issued' AND i.deleted_at IS NULL AND i.discount > 0 AND i.local_date BETWEEN ?1 AND ?2
             ORDER BY i.local_date, i.number {CAP}"
        ),
        &[&c.from, &c.to],
        |r| {
            let (subtotal, discount): (i64, i64) = (r.get(3)?, r.get(4)?);
            let share = if subtotal > 0 { discount * 10_000 / subtotal } else { 0 };
            Ok(vec![date(r.get::<_, String>(0)?), code(r.get::<_, String>(1)?), text(r.get::<_, String>(2)?), money(subtotal), money(discount), ReportCell::Percent { value: share }])
        },
    )?;
    Ok(table(&["date", "number", "patient", "subtotal", "discount", "share"], rows, &[3, 4]))
}

fn refunds(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        &format!(
            "SELECT p.local_date, p.number, pt.full_name, COALESCE(m.name_fa, ''), COALESCE(m.name_ps, ''), COALESCE(m.name_en, ''), p.amount, p.reason
             FROM payment p JOIN patient pt ON pt.id = p.patient_id LEFT JOIN payment_method m ON m.id = p.method_id
             WHERE p.kind = 'refund' AND p.local_date BETWEEN ?1 AND ?2 ORDER BY p.local_date, p.number {CAP}"
        ),
        &[&c.from, &c.to],
        |r| {
            Ok(vec![
                date(r.get::<_, String>(0)?),
                code(r.get::<_, String>(1)?),
                text(r.get::<_, String>(2)?),
                name(r.get(3)?, r.get(4)?, r.get(5)?),
                money(r.get(6)?),
                opt_text(r.get(7)?),
            ])
        },
    )?;
    Ok(table(&["date", "number", "patient", "method", "amount", "reason"], rows, &[4]))
}

fn doctor_revenue(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        "SELECT COALESCE(d.full_name, ''), COUNT(DISTINCT i.patient_id), COALESCE(SUM(l.quantity), 0), COALESCE(SUM(l.line_total), 0)
         FROM invoice_line l JOIN invoice i ON i.id = l.invoice_id LEFT JOIN doctor d ON d.id = l.doctor_id
         WHERE i.status = 'issued' AND i.deleted_at IS NULL AND i.local_date BETWEEN ?1 AND ?2
           AND (?3 IS NULL OR l.doctor_id = ?3)
         GROUP BY l.doctor_id ORDER BY 4 DESC",
        &[&c.from, &c.to, &c.doctor],
        |r| Ok(vec![opt_text(r.get(0)?), int(r.get(1)?), int(r.get(2)?), money(r.get(3)?)]),
    )?;
    Ok(table(&["doctor", "patients", "units", "invoiced"], rows, &[2, 3]))
}

fn doctor_commission(c: &Ctx) -> Result<Table> {
    let report = billing::commission_report(
        c.conn,
        &CommissionReportParams { doctor_id: c.doctor.clone(), from: c.from.clone(), to: c.to.clone() },
    )?;
    let rows = report
        .doctors
        .into_iter()
        .map(|d| vec![text(d.doctor_name), int(d.lines.len() as i64), money(d.base), money(d.commission)])
        .collect();
    Ok(table(&["doctor", "lines", "base", "commission"], rows, &[1, 2, 3]))
}

fn service_revenue(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        "SELECT s.name_fa, s.name_ps, s.name_en, COALESCE(SUM(l.quantity), 0), COALESCE(SUM(l.line_total), 0)
         FROM invoice_line l JOIN invoice i ON i.id = l.invoice_id JOIN service s ON s.id = l.service_id
         WHERE i.status = 'issued' AND i.deleted_at IS NULL AND i.local_date BETWEEN ?1 AND ?2
           AND (?3 IS NULL OR l.service_id = ?3) AND (?4 IS NULL OR l.doctor_id = ?4)
         GROUP BY s.id ORDER BY 5 DESC",
        &[&c.from, &c.to, &c.service, &c.doctor],
        |r| Ok(vec![name(r.get(0)?, r.get(1)?, r.get(2)?), int(r.get(3)?), money(r.get(4)?)]),
    )?;
    Ok(table(&["service", "units", "invoiced"], rows, &[1, 2]))
}

fn expenses(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        &format!(
            "SELECT e.local_date, k.name_fa, k.name_ps, k.name_en, e.description, e.payee, e.amount
             FROM expense e JOIN expense_category k ON k.id = e.category_id
             WHERE e.deleted_at IS NULL AND e.local_date BETWEEN ?1 AND ?2 ORDER BY e.local_date, e.created_at {CAP}"
        ),
        &[&c.from, &c.to],
        |r| Ok(vec![date(r.get::<_, String>(0)?), name(r.get(1)?, r.get(2)?, r.get(3)?), text(r.get::<_, String>(4)?), opt_text(r.get(5)?), money(r.get(6)?)]),
    )?;
    Ok(table(&["date", "category", "description", "payee", "amount"], rows, &[4]))
}

/// Profit and loss by month: money received, minus refunds, minus everything the clinic spent.
fn profit_loss(c: &Ctx) -> Result<Table> {
    let mut months: BTreeMap<String, (i64, i64, i64)> = BTreeMap::new();
    for (d, m) in day_money(c)? {
        let e = months.entry(month_key(&d, c.calendar)).or_default();
        e.0 += m.received;
        e.1 += m.refunded;
    }
    let mut stmt = c.conn.prepare(
        "SELECT local_date, SUM(amount) FROM expense WHERE deleted_at IS NULL AND local_date BETWEEN ?1 AND ?2 GROUP BY local_date",
    )?;
    for row in stmt.query_map([&c.from, &c.to], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        let (d, amount) = row?;
        months.entry(month_key(&d, c.calendar)).or_default().2 += amount;
    }
    let rows = months
        .into_iter()
        .map(|(k, (received, refunded, spent))| {
            vec![
                ReportCell::Month { value: k },
                money(received),
                money(refunded),
                money(spent),
                money(received - refunded - spent),
            ]
        })
        .collect();
    Ok(table(&["month", "received", "refunded", "expenses", "profit"], rows, &[1, 2, 3, 4]))
}

fn cash_closings(c: &Ctx) -> Result<Table> {
    let rows = collect(
        c,
        &format!(
            "SELECT k.local_date, k.expected, k.counted, k.difference, k.reason, u.display_name
             FROM cash_closing k LEFT JOIN app_user u ON u.id = k.closed_by
             WHERE k.local_date BETWEEN ?1 AND ?2 ORDER BY k.local_date {CAP}"
        ),
        &[&c.from, &c.to],
        |r| {
            Ok(vec![
                date(r.get::<_, String>(0)?),
                money(r.get(1)?),
                money(r.get(2)?),
                money(r.get(3)?),
                opt_text(r.get(4)?),
                opt_text(r.get(5)?),
            ])
        },
    )?;
    Ok(table(&["date", "expected", "counted", "difference", "reason", "closed_by"], rows, &[1, 2, 3]))
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
    let file_name = format!("report-{kind}-{stamp}.xlsx");
    std::fs::create_dir_all(exports_dir)?;
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

/// What needs attention right now for this user. Nothing is stored except which keys were read.
pub fn notifications(conn: &Connection, s: &Session) -> Result<NotificationList> {
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
        for d in billing::installments_due(conn, &InstallmentsDueParams::default())?.into_iter().take(50) {
            let level = if d.overdue { L::Danger } else { L::Warning };
            items.push(item(
                format!("installment_due:{}:{}", d.invoice_id, d.no),
                K::InstallmentDue,
                level,
                same(&d.patient_name),
                Some(d.unpaid.to_string()),
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

pub fn mark_read(conn: &Connection, s: &Session, p: &NotificationKeysParams) -> Result<NotificationList> {
    let keys: Vec<String> = if p.keys.is_empty() {
        notifications(conn, s)?.items.into_iter().map(|i| i.key).collect()
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
    notifications(conn, s)
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
        for (kind, table_name, date_col, order_col) in [
            (SearchKind::Invoice, "invoice", "local_date", "issued_at"),
            (SearchKind::Receipt, "payment", "local_date", "paid_at"),
        ] {
            let extra = if kind == SearchKind::Invoice {
                "AND t.deleted_at IS NULL"
            } else {
                "AND t.kind = 'payment'"
            };
            let mut stmt = conn.prepare(&format!(
                "SELECT t.id, t.patient_id, t.number, p.full_name, t.{date_col}
                 FROM {table_name} t JOIN patient p ON p.id = t.patient_id
                 WHERE ((t.number >= ? AND t.number < ?) OR t.patient_id IN ({marks})) {extra}
                 ORDER BY t.{order_col} DESC LIMIT ?"
            ))?;
            let args = with_ids(vec![Value::Text(number_lo.clone()), Value::Text(number_hi.clone())]);
            let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
                Ok(SearchHit {
                    kind,
                    id: r.get(0)?,
                    patient_id: Some(r.get(1)?),
                    title: r.get(2)?,
                    subtitle: Some(r.get(3)?),
                    date: Some(r.get(4)?),
                    page: "billing".into(),
                })
            })?;
            for h in rows {
                hits.push(h?);
            }
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
