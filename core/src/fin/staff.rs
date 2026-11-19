//! Employees and the monthly payroll (PR). A payroll books what each employee earned in a period
//! (Dr 5110 salaries, gross) against what the clinic now owes them (Cr 2310, net), the salary tax it
//! withheld (Cr 2410, entered by hand) and the advances it takes back (Cr 1320). Salaries and
//! advances are paid with payment vouchers (`spend::create_voucher`, purposes salary and advance).

use std::collections::HashSet;

use artaveo_shared::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::new_id;

use super::ledger::*;
use super::shares::{period_key, period_of, OVERLAP_SQL};
use super::util::*;

// ───────────────────────────── employees ─────────────────────────────

const EMPLOYEE_SELECT: &str = "SELECT e.id, e.full_name, e.father_name, e.tin, e.national_id, e.phone, e.position,
    e.doctor_id, e.hire_date, e.end_date, e.base_salary, e.note, e.is_active, e.version,
    -COALESCE((SELECT SUM(g.amount) FROM fin_gl g WHERE g.account_id = 2310 AND g.party_id = e.id), 0),
    COALESCE((SELECT SUM(g.amount) FROM fin_gl g WHERE g.account_id = 1320 AND g.party_id = e.id), 0)
    FROM fin_employee e";

fn map_employee(r: &rusqlite::Row<'_>) -> rusqlite::Result<EmployeeInfo> {
    Ok(EmployeeInfo {
        id: r.get(0)?,
        full_name: r.get(1)?,
        father_name: r.get(2)?,
        tin: r.get(3)?,
        national_id: r.get(4)?,
        phone: r.get(5)?,
        position: r.get(6)?,
        doctor_id: r.get(7)?,
        hire_date: iso_of(r.get(8)?),
        end_date: r.get::<_, Option<i64>>(9)?.map(iso_of),
        base_salary: r.get(10)?,
        note: r.get(11)?,
        is_active: r.get::<_, i64>(12)? == 1,
        version: r.get(13)?,
        owed: r.get(14)?,
        advances: r.get(15)?,
    })
}

pub fn list_employees(conn: &Connection, p: &EmployeeListParams) -> Result<Vec<EmployeeInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{EMPLOYEE_SELECT} WHERE ?1 OR e.is_active = 1 ORDER BY e.is_active DESC, e.full_name"
    ))?;
    let rows = stmt.query_map([p.include_inactive], map_employee)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn get_employee(conn: &Connection, id: &str) -> Result<EmployeeInfo> {
    conn.query_row(&format!("{EMPLOYEE_SELECT} WHERE e.id = ?1"), [id], map_employee)
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("employee {id}"))
                .on_field("employee_id", ValidationRule::EmployeeNotFound)
        })
}

fn clean_tin(field: &str, v: &Option<String>) -> Result<Option<String>> {
    let t = optional(field, v, 20)?;
    if t.as_deref().is_some_and(|t| !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
        return Err(CoreError::invalid(field, ValidationRule::InvalidParams, "digits and letters only"));
    }
    Ok(t)
}

pub fn save_employee(conn: &Connection, actor: &Actor, p: &SaveEmployeeParams) -> Result<EmployeeInfo> {
    let full_name = required("full_name", &p.full_name, 100)?;
    let position = required("position", &p.position, 60)?;
    let father_name = optional("father_name", &p.father_name, 100)?;
    let tin = clean_tin("tin", &p.tin)?;
    let national_id = optional("national_id", &p.national_id, 30)?;
    let phone = optional("phone", &p.phone, 30)?;
    let note = optional("note", &p.note, 300)?;
    let doctor = check_doctor(conn, &p.doctor_id)?;
    let hire = ymd(&date_of("hire_date", &p.hire_date)?);
    let end = opt_date("end_date", &p.end_date)?.map(|d| ymd(&d));
    if end.is_some_and(|e| e < hire) {
        return Err(CoreError::invalid("end_date", ValidationRule::PeriodRange, "the end is before the start"));
    }
    let base = money("base_salary", p.base_salary, true)?;
    let now = now_secs();
    let id = match p.id.as_deref().filter(|s| !s.is_empty()) {
        Some(id) => {
            let before = get_employee(conn, id)?;
            let changed = conn.execute(
                "UPDATE fin_employee SET full_name = ?1, father_name = ?2, tin = ?3, national_id = ?4, phone = ?5,
                    position = ?6, doctor_id = ?7, hire_date = ?8, end_date = ?9, base_salary = ?10, note = ?11,
                    is_active = ?12, updated_at = ?13, version = version + 1
                 WHERE id = ?14 AND version = ?15",
                params![
                    full_name,
                    father_name,
                    tin,
                    national_id,
                    phone,
                    position,
                    doctor,
                    hire,
                    end,
                    base,
                    note,
                    p.is_active as i64,
                    now,
                    id,
                    p.version.unwrap_or(before.version)
                ],
            )?;
            expect_one_row(changed, "employee")?;
            audit::record(
                conn,
                actor,
                "employee.update",
                Some("employee"),
                Some(id),
                Some(&json!({"name": before.full_name, "base_salary": before.base_salary, "active": before.is_active})),
                Some(&json!({"name": full_name, "base_salary": base, "active": p.is_active})),
            )?;
            id.to_string()
        }
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO fin_employee(id, full_name, father_name, tin, national_id, phone, position, doctor_id,
                    hire_date, end_date, base_salary, note, is_active, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    id,
                    full_name,
                    father_name,
                    tin,
                    national_id,
                    phone,
                    position,
                    doctor,
                    hire,
                    end,
                    base,
                    note,
                    p.is_active as i64,
                    now
                ],
            )?;
            audit::record(
                conn,
                actor,
                "employee.create",
                Some("employee"),
                Some(&id),
                None,
                Some(&json!({"name": full_name, "position": position, "base_salary": base})),
            )?;
            id
        }
    };
    get_employee(conn, &id)
}

// ───────────────────────────── payroll ─────────────────────────────

/// One checked line of a payroll being approved.
struct PayRow {
    employee: String,
    base: i64,
    extra: i64,
    commission: i64,
    advance: i64,
    tax: i64,
    note: Option<String>,
}

impl PayRow {
    fn gross(&self) -> i64 {
        self.base + self.extra + self.commission
    }
}

/// `(year, serial, date, period, status, void reason, created by)` of a payroll header.
type PayrollHead = (i64, i64, i64, Option<String>, String, Option<String>, Option<String>);

fn totals(lines: &[PayrollLineInfo]) -> (i64, i64, i64, i64) {
    lines.iter().fold((0, 0, 0, 0), |(g, a, t, n), l| (g + l.gross, a + l.advance, t + l.tax, n + l.net))
}

/// The payroll that covers part of a period, if one was approved.
fn posted_in(conn: &Connection, from: i64, to: i64) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM fin_doc WHERE kind = 'pr' AND status = 'posted'
                AND CAST(substr(ext_id, 1, 8) AS INTEGER) <= ?2 AND CAST(substr(ext_id, 10, 8) AS INTEGER) >= ?1
             ORDER BY id DESC LIMIT 1",
            params![from, to],
            |r| r.get(0),
        )
        .optional()?)
}

/// What a payroll for the period would hold: the approved one when there is one, otherwise a line
/// for every employee working in the period at their base salary (the screen edits it).
pub fn draft(conn: &Connection, p: &PayrollDraftParams) -> Result<PayrollInfo> {
    let (from, to) = period(&p.from, &p.to)?;
    if let Some(id) = posted_in(conn, from, to)? {
        return get(conn, id);
    }
    let mut stmt = conn.prepare(&format!(
        "{EMPLOYEE_SELECT} WHERE e.is_active = 1 AND e.hire_date <= ?2 AND (e.end_date IS NULL OR e.end_date >= ?1)
         ORDER BY e.full_name"
    ))?;
    let employees: Vec<EmployeeInfo> =
        stmt.query_map(params![from, to], map_employee)?.collect::<rusqlite::Result<_>>()?;
    let lines: Vec<PayrollLineInfo> = employees
        .into_iter()
        .map(|e| PayrollLineInfo {
            employee_id: e.id,
            full_name: e.full_name,
            father_name: e.father_name,
            tin: e.tin,
            position: e.position,
            base: e.base_salary,
            extra: 0,
            commission: 0,
            gross: e.base_salary,
            advance: 0,
            tax: 0,
            net: e.base_salary,
            paid: 0,
            advances_open: e.advances,
            note: None,
        })
        .collect();
    let (gross, advance, tax, net) = totals(&lines);
    Ok(PayrollInfo {
        id: None,
        number: None,
        from: iso_of(from),
        to: iso_of(to),
        date: None,
        status: None,
        lines,
        gross,
        advance,
        tax,
        net,
        void_reason: None,
        created_by_name: None,
    })
}

/// Approves a payroll for a period (one per period): Dr 5110 gross; Cr 2310 net, 2410 tax and 1320
/// advance per employee.
pub fn create(conn: &Connection, actor: &Actor, p: &CreatePayrollParams) -> Result<PayrollInfo> {
    let (from, to) = period(&p.from, &p.to)?;
    let date = doc_day("date", &p.date)?;
    let note = optional("note", &p.note, 300)?;
    let taken: bool = conn.query_row(OVERLAP_SQL, params![None::<String>, from, to, "pr"], |r| r.get(0))?;
    if taken {
        return Err(CoreError::invalid(
            "from",
            ValidationRule::PeriodTaken,
            "a payroll already covers part of this period",
        ));
    }
    if p.lines.is_empty() {
        return Err(CoreError::invalid("lines", ValidationRule::Required, "the payroll has no employee"));
    }
    if p.lines.len() > 500 {
        return Err(CoreError::invalid("lines", ValidationRule::InvalidParams, "too many lines"));
    }
    let mut seen = HashSet::new();
    let mut rows: Vec<PayRow> = Vec::new();
    for (i, l) in p.lines.iter().enumerate() {
        let field = |f: &str| format!("lines.{i}.{f}");
        let e = get_employee(conn, &l.employee_id).map_err(|e| e.on_field(&field("employee_id"), ValidationRule::EmployeeNotFound))?;
        if !seen.insert(e.id.clone()) {
            return Err(CoreError::invalid(&field("employee_id"), ValidationRule::ItemDuplicate, "an employee is twice on the payroll"));
        }
        let base = money(&field("base"), l.base, true)?;
        let extra = money(&field("extra"), l.extra, true)?;
        let commission = money(&field("commission"), l.commission, true)?;
        let advance = money(&field("advance"), l.advance, true)?;
        let tax = money(&field("tax"), l.tax, true)?;
        let gross = base + extra + commission;
        if gross > MAX_AMOUNT {
            return Err(CoreError::invalid(&field("base"), ValidationRule::BillingAmount, "the salary is too large"));
        }
        if advance > e.advances {
            return Err(CoreError::invalid(
                &field("advance"),
                ValidationRule::AdvanceExceeds,
                format!("{} has {} in advances to take back", e.full_name, e.advances),
            ));
        }
        if tax + advance > gross {
            return Err(CoreError::invalid(&field("tax"), ValidationRule::WithheldRange, "the deductions are more than the salary"));
        }
        if gross == 0 && advance == 0 {
            continue;
        }
        let note = optional(&field("note"), &l.note, 200)?;
        rows.push(PayRow { employee: e.id, base, extra, commission, advance, tax, note });
    }
    if rows.is_empty() {
        return Err(CoreError::invalid("lines", ValidationRule::Required, "the payroll has no salary"));
    }
    let gross_total: i64 = rows.iter().map(PayRow::gross).sum();
    let tax_total: i64 = rows.iter().map(|r| r.tax).sum();
    let net_total: i64 = rows.iter().map(|r| r.gross() - r.advance - r.tax).sum();
    let key = period_key(from, to);
    let (id, number) = insert_doc(
        conn,
        actor,
        &NewDoc {
            kind: Some(DocKind::Pr),
            date,
            amount: net_total,
            ext_id: Some(&key),
            note: note.as_deref(),
            ..Default::default()
        },
    )?;
    let mut entries = vec![dr(SALARIES, gross_total), cr(TAX_WAGE, tax_total)];
    for r in &rows {
        conn.execute(
            "INSERT INTO fin_pay_line(doc_id, employee_id, base, extra, commission, advance, tax, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, r.employee, r.base, r.extra, r.commission, r.advance, r.tax, r.note],
        )?;
        entries.push(cr(WAGES_PAYABLE, r.gross() - r.advance - r.tax).of(&r.employee));
        entries.push(cr(ADVANCES, r.advance).of(&r.employee));
    }
    post(conn, id, date, entries)?;
    audit::record(
        conn,
        actor,
        "payroll.approve",
        Some("fin_doc"),
        Some(&id.to_string()),
        None,
        Some(&json!({"number": number, "from": iso_of(from), "to": iso_of(to), "gross": gross_total, "net": net_total})),
    )?;
    get(conn, id)
}

pub fn get(conn: &Connection, id: i64) -> Result<PayrollInfo> {
    let (year, serial, date, key, status, void_reason, by): PayrollHead = conn
        .query_row(
            "SELECT year, serial, doc_date, ext_id, status, void_reason, created_by FROM fin_doc WHERE id = ?1 AND kind = 'pr'",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("payroll {id}")).on_field("id", ValidationRule::DocNotFound)
        })?;
    let mut stmt = conn.prepare_cached(
        "SELECT l.employee_id, COALESCE(e.full_name, ''), e.father_name, e.tin, COALESCE(e.position, ''),
            l.base, l.extra, l.commission, l.advance, l.tax, l.note,
            COALESCE((SELECT SUM(g.amount) FROM fin_gl g JOIN fin_doc v ON v.id = g.doc_id
                WHERE g.account_id = 2310 AND g.party_id = l.employee_id AND v.kind = 'pv'), 0)
         FROM fin_pay_line l LEFT JOIN fin_employee e ON e.id = l.employee_id WHERE l.doc_id = ?1 ORDER BY e.full_name",
    )?;
    let lines: Vec<PayrollLineInfo> = stmt
        .query_map([id], |r| {
            let base: i64 = r.get(5)?;
            let extra: i64 = r.get(6)?;
            let commission: i64 = r.get(7)?;
            let advance: i64 = r.get(8)?;
            let tax: i64 = r.get(9)?;
            let gross = base + extra + commission;
            Ok(PayrollLineInfo {
                employee_id: r.get(0)?,
                full_name: r.get(1)?,
                father_name: r.get(2)?,
                tin: r.get(3)?,
                position: r.get(4)?,
                base,
                extra,
                commission,
                gross,
                advance,
                tax,
                net: gross - advance - tax,
                paid: r.get(11)?,
                advances_open: 0,
                note: r.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let (gross, advance, tax, net) = totals(&lines);
    let (from, to) = period_of(&key);
    Ok(PayrollInfo {
        id: Some(id),
        number: Some(format_number("PR", year, serial)),
        from: iso_of(from),
        to: iso_of(to),
        date: Some(iso_of(date)),
        status: Some(status_of(&status)),
        lines,
        gross,
        advance,
        tax,
        net,
        void_reason,
        created_by_name: user_name(conn, &by)?,
    })
}

pub fn list(conn: &Connection, p: &PayrollListParams) -> Result<Vec<PayrollInfo>> {
    let from = opt_date("from", &p.from)?.map(|d| ymd(&d)).unwrap_or(0);
    let to = opt_date("to", &p.to)?.map(|d| ymd(&d)).unwrap_or(99_991_231);
    let mut stmt = conn.prepare(
        "SELECT id FROM fin_doc WHERE kind = 'pr' AND doc_date BETWEEN ?1 AND ?2 ORDER BY doc_date DESC, id DESC LIMIT 200",
    )?;
    let ids: Vec<i64> = stmt.query_map(params![from, to], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    ids.into_iter().map(|id| get(conn, id)).collect()
}

// ───────────────────────────── checks after a void ─────────────────────────────

/// After a payroll, statement or staff voucher is voided: nobody may end up owing the clinic salary
/// it already paid (2310 debit), advances taken back that were never given (1320 credit), shares
/// paid but not approved (2320 debit), or withheld salary tax paid but not withheld (2410 debit).
pub(crate) fn check_staff_books(conn: &Connection) -> Result<()> {
    let bad: i64 = scalar(
        conn,
        "SELECT COUNT(*) FROM (SELECT account_id, party_id, SUM(amount) AS b FROM fin_gl
            WHERE account_id IN (1320, 2310, 2320) AND party_id IS NOT NULL GROUP BY account_id, party_id)
         WHERE (account_id IN (2310, 2320) AND b > 0) OR (account_id = 1320 AND b < 0)",
        &[],
    )?;
    let wage_tax = balance(conn, TAX_WAGE, None, 99_991_231)?;
    if bad > 0 || wage_tax > 0 {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::DocNotVoidable,
            "money was already paid (or taken back) on this document: void those payments first",
        ));
    }
    Ok(())
}
