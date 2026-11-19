//! The finance overview of a period: money that moved (cash method), work invoiced (accrual method),
//! balances at the end, and the checks that prove the books agree with themselves.

use std::collections::BTreeMap;

use artaveo_shared::*;
use rusqlite::{params, Connection};

use crate::error::Result;

use super::ledger::*;
use super::util::*;

/// `(doc kind — `pv:<purpose>` for payment vouchers —, account, account type, money kind, Σ amount)`
/// of the ledger rows of a period.
type Flow = (String, i64, String, Option<String>, i64);

fn flows(conn: &Connection, from: i64, to: i64) -> Result<Vec<Flow>> {
    let mut stmt = conn.prepare(
        "SELECT CASE WHEN d.kind = 'pv' THEN 'pv:' || d.purpose ELSE d.kind END AS k, g.account_id, a.type, a.money,
            SUM(g.amount)
         FROM fin_gl g JOIN fin_doc d ON d.id = g.doc_id JOIN fin_account a ON a.id = g.account_id
         WHERE g.date BETWEEN ?1 AND ?2 GROUP BY k, g.account_id",
    )?;
    let rows = stmt
        .query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<rusqlite::Result<Vec<Flow>>>()?;
    Ok(rows)
}

fn sum(rows: &[Flow], keep: impl Fn(&Flow) -> bool) -> i64 {
    rows.iter().filter(|r| keep(r)).map(|r| r.4).sum()
}

fn is_pv(r: &Flow) -> bool {
    r.0.starts_with("pv")
}

fn is_tax(account: i64) -> bool {
    matches!(account, TAX_WAGE | TAX_RENT | TAX_CONTRACTOR)
}

fn by_expense_account(rows: &[Flow], keep: impl Fn(&Flow) -> bool) -> Vec<AccountTotal> {
    let mut m: BTreeMap<i64, i64> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.2 == "expense" && r.1 != BAD_DEBT && keep(r)) {
        *m.entry(r.1).or_default() += r.4;
    }
    m.into_iter()
        .filter(|(_, v)| *v != 0)
        .map(|(account_id, amount)| AccountTotal { account_id, amount })
        .collect()
}

fn cash_basis(conn: &Connection, rows: &[Flow], from: i64, to: i64) -> Result<CashBasis> {
    let money = |r: &Flow| r.3.is_some();
    let received = sum(rows, |r| r.0 == "rc" && money(r));
    let refunded = -sum(rows, |r| r.0 == "rf" && money(r));
    let expenses = by_expense_account(rows, is_pv);
    let expenses_total: i64 = expenses.iter().map(|e| e.amount).sum();
    let suppliers_paid = sum(rows, |r| is_pv(r) && r.1 == PAYABLE);
    let staff_paid = sum(rows, |r| is_pv(r) && matches!(r.1, WAGES_PAYABLE | ADVANCES | SHARES_PAYABLE));
    let net_received = received - refunded;
    // Money accounts: opening, in, out, closing.
    let mut stmt = conn.prepare("SELECT id FROM fin_account WHERE money IS NOT NULL ORDER BY id")?;
    let ids: Vec<i64> = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let mut money_rows = Vec::new();
    for id in ids {
        let (inflow, outflow): (i64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN amount > 0 THEN amount END), 0), COALESCE(-SUM(CASE WHEN amount < 0 THEN amount END), 0)
             FROM fin_gl WHERE account_id = ?1 AND date BETWEEN ?2 AND ?3",
            params![id, from, to],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let opening = balance(conn, id, None, from - 1)?;
        if opening == 0 && inflow == 0 && outflow == 0 {
            continue;
        }
        money_rows.push(MoneyAccountTotal {
            account_id: id,
            opening,
            inflow,
            outflow,
            closing: opening + inflow - outflow,
        });
    }
    Ok(CashBasis {
        received,
        refunded,
        net_received,
        expenses,
        expenses_total,
        suppliers_paid,
        staff_paid,
        operating: net_received - expenses_total - suppliers_paid - staff_paid,
        withheld: -sum(rows, |r| is_pv(r) && r.0 != "pv:tax" && is_tax(r.1)),
        owner_in: -sum(rows, |r| r.0 == "or" && r.1 == CAPITAL),
        owner_out: sum(rows, |r| is_pv(r) && r.1 == DRAWINGS),
        loans_in: -sum(rows, |r| r.0 == "or" && r.1 == LOANS),
        loans_out: sum(rows, |r| is_pv(r) && r.1 == LOANS),
        tax_paid: sum(rows, |r| r.0 == "pv:tax" && is_tax(r.1)),
        other_income: -sum(rows, |r| r.0 == "or" && r.1 == OTHER_INCOME),
        count_differences: sum(rows, |r| r.0 == "cc" && money(r)),
        money_opening: money_rows.iter().map(|m| m.opening).sum(),
        money_closing: money_rows.iter().map(|m| m.closing).sum(),
        money: money_rows,
    })
}

fn accrual_basis(rows: &[Flow]) -> AccrualBasis {
    let gross = -sum(rows, |r| r.1 == INCOME);
    let discounts = sum(rows, |r| r.1 == DISCOUNTS);
    let reductions = sum(rows, |r| r.1 == REDUCTIONS);
    let other_income = -sum(rows, |r| r.1 == OTHER_INCOME);
    let expenses = by_expense_account(rows, |_| true);
    let expenses_total: i64 = expenses.iter().map(|e| e.amount).sum();
    let write_offs = sum(rows, |r| r.1 == BAD_DEBT);
    let revenue = gross - discounts - reductions + other_income;
    AccrualBasis {
        gross,
        discounts,
        reductions,
        other_income,
        revenue,
        expenses,
        expenses_total,
        write_offs,
        result: revenue - expenses_total - write_offs,
    }
}

fn check(key: &str, left: i64, right: i64) -> CheckResult {
    CheckResult { key: key.to_string(), ok: left == right, left, right }
}

/// The checks that must always hold, whatever was written: each ledger agrees with its documents.
pub(crate) fn standing_checks(conn: &Connection) -> Result<Vec<CheckResult>> {
    let gl_total = scalar(conn, "SELECT SUM(amount) FROM fin_gl", &[])?;
    let unbalanced = super::docs::unbalanced_docs(conn)?;
    let invoices_left: i64 = {
        let mut stmt = conn.prepare("SELECT id FROM fin_doc WHERE kind = 'inv' AND status = 'posted'")?;
        let ids: Vec<i64> = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let mut total = 0;
        for id in ids {
            total += super::invoices::settled(conn, id)?.remaining();
        }
        total
    };
    let deposits_docs = scalar(
        conn,
        "SELECT (SELECT COALESCE(SUM(amount), 0) FROM fin_doc WHERE kind = 'rc' AND status = 'posted')
              - (SELECT COALESCE(SUM(l.amount), 0) FROM fin_line l JOIN fin_doc d ON d.id = l.doc_id
                    WHERE d.kind IN ('rc', 'al') AND d.status = 'posted')
              - (SELECT COALESCE(SUM(amount), 0) FROM fin_doc WHERE kind = 'rf' AND status = 'posted')",
        &[],
    )?;
    let payables_docs = scalar(
        conn,
        "SELECT (SELECT COALESCE(SUM(amount), 0) FROM fin_doc WHERE kind = 'bl' AND status = 'posted')
              - (SELECT COALESCE(SUM(l.amount), 0) FROM fin_line l JOIN fin_doc d ON d.id = l.doc_id
                    WHERE d.kind = 'pv' AND d.status = 'posted' AND l.account_id = ?1)",
        &[&PAYABLE],
    )?;
    Ok(vec![
        check("ledger_balanced", gl_total + unbalanced, 0),
        check(
            "patients",
            invoices_left,
            scalar(conn, "SELECT SUM(amount) FROM fin_gl WHERE account_id = ?1", &[&RECEIVABLE])?,
        ),
        check(
            "deposits",
            deposits_docs,
            -scalar(conn, "SELECT SUM(amount) FROM fin_gl WHERE account_id = ?1", &[&DEPOSITS])?,
        ),
        check(
            "suppliers",
            payables_docs,
            -scalar(conn, "SELECT SUM(amount) FROM fin_gl WHERE account_id = ?1", &[&PAYABLE])?,
        ),
    ])
}

pub fn overview(conn: &Connection, p: &PeriodParams) -> Result<FinanceOverview> {
    let (from, to) = period(&p.from, &p.to)?;
    let rows = flows(conn, from, to)?;
    let cash = cash_basis(conn, &rows, from, to)?;
    let accrual = accrual_basis(&rows);
    let receivables = balance(conn, RECEIVABLE, None, to)?;
    let deposits = -balance(conn, DEPOSITS, None, to)?;
    let ar_change = receivables - balance(conn, RECEIVABLE, None, from - 1)?;
    let dep_change = deposits + balance(conn, DEPOSITS, None, from - 1)?;
    let write_offs = accrual.write_offs;
    let patient_revenue = accrual.gross - accrual.discounts - accrual.reductions;
    let mut checks = standing_checks(conn)?;
    checks.push(check("bridge", patient_revenue - write_offs - ar_change + dep_change, cash.net_received));
    // Paid for something that is neither an expense nor a debt (equipment).
    let assets_bought = sum(&rows, |r| is_pv(r) && r.3.is_none() && r.2 == "asset" && r.1 != ADVANCES);
    let explained = cash.net_received - assets_bought - cash.expenses_total - cash.suppliers_paid
        - cash.staff_paid
        + cash.withheld
        - cash.tax_paid
        + cash.owner_in
        - cash.owner_out
        + cash.loans_in
        - cash.loans_out
        + cash.other_income
        + cash.count_differences;
    checks.push(check("money", explained, cash.money_closing - cash.money_opening));
    let overdue = super::patients::open_installments(conn, None)?
        .into_iter()
        .filter(|i| i.state == InstallmentState::Overdue)
        .map(|i| i.amount - i.paid)
        .sum();
    Ok(FinanceOverview {
        from: iso_of(from),
        to: iso_of(to),
        receivables,
        deposits,
        payables: -balance(conn, PAYABLE, None, to)?,
        tax_withheld: -(balance(conn, TAX_WAGE, None, to)?
            + balance(conn, TAX_RENT, None, to)?
            + balance(conn, TAX_CONTRACTOR, None, to)?),
        overdue_installments: overdue,
        checks,
        cash,
        accrual,
    })
}
