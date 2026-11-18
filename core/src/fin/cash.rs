//! The cash desk: one money account on one day, and cash counts (CC). A count compares the money in
//! the drawer with the books; a difference is booked to over/short (5910) with its reason.

use artaveo_shared::*;
use rusqlite::{params, Connection};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::error::{CoreError, Result};

use super::ledger::*;
use super::util::*;

const COUNT_SELECT: &str = "SELECT d.id, d.year, d.serial, d.account_id, d.doc_date, d.created_at, d.amount, d.reason, d.status,
    d.created_by, (SELECT COALESCE(SUM(g.amount), 0) FROM fin_gl g WHERE g.doc_id = d.id AND g.account_id = d.account_id
        AND g.rev = 0)
    FROM fin_doc d WHERE d.kind = 'cc'";

fn map_count(r: &rusqlite::Row<'_>) -> rusqlite::Result<(CashCountInfo, Option<String>)> {
    let counted: i64 = r.get(6)?;
    let difference: i64 = r.get(10)?;
    Ok((
        CashCountInfo {
            id: r.get(0)?,
            number: format_number("CC", r.get(1)?, r.get(2)?),
            account_id: r.get::<_, Option<i64>>(3)?.unwrap_or(CASH),
            date: iso_of(r.get(4)?),
            created_at: iso_at(r.get(5)?),
            book: counted - difference,
            counted,
            difference,
            reason: r.get(7)?,
            status: status_of(&r.get::<_, String>(8)?),
            created_by_name: None,
        },
        r.get(9)?,
    ))
}

fn named(conn: &Connection, rows: Vec<(CashCountInfo, Option<String>)>) -> Result<Vec<CashCountInfo>> {
    rows.into_iter()
        .map(|(mut c, by)| {
            c.created_by_name = user_name(conn, &by)?;
            Ok(c)
        })
        .collect()
}

pub fn counts(conn: &Connection, p: &CashCountListParams) -> Result<Vec<CashCountInfo>> {
    let to = ymd(&opt_date("to", &p.to)?.unwrap_or_else(crate::clock::today_iso));
    let from = match opt_date("from", &p.from)? {
        Some(f) => ymd(&f),
        None => super::invoices::days_after(to, -60),
    };
    let mut stmt = conn.prepare(&format!(
        "{COUNT_SELECT} AND d.doc_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR d.account_id = ?3) ORDER BY d.doc_date DESC, d.id DESC"
    ))?;
    let rows = stmt
        .query_map(params![from, to, p.account_id], map_count)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    named(conn, rows)
}

fn counts_of_day(conn: &Connection, account: i64, date: i64) -> Result<Vec<CashCountInfo>> {
    let mut stmt =
        conn.prepare(&format!("{COUNT_SELECT} AND d.doc_date = ?1 AND d.account_id = ?2 AND d.status = 'posted' ORDER BY d.id"))?;
    let rows = stmt.query_map(params![date, account], map_count)?.collect::<rusqlite::Result<Vec<_>>>()?;
    named(conn, rows)
}

/// One money account on one day: where the money came from and went.
pub fn day(conn: &Connection, p: &CashDayParams) -> Result<CashDayInfo> {
    let account = money_account(conn, "account_id", p.account_id)?;
    let date = ymd(&date_of("date", &p.date)?);
    let opening = balance(conn, account, None, date - 1)?;
    // The account's movements of the day by the kind of document.
    let mut stmt = conn.prepare(
        "SELECT d.kind, COALESCE(SUM(g.amount), 0) FROM fin_gl g JOIN fin_doc d ON d.id = g.doc_id
         WHERE g.account_id = ?1 AND g.date = ?2 GROUP BY d.kind",
    )?;
    let by_kind: Vec<(String, i64)> =
        stmt.query_map(params![account, date], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    let sum = |kinds: &[&str]| by_kind.iter().filter(|(k, _)| kinds.contains(&k.as_str())).map(|(_, a)| a).sum::<i64>();
    let receipt_count = scalar(
        conn,
        "SELECT COUNT(*) FROM fin_doc WHERE kind = 'rc' AND account_id = ?1 AND doc_date = ?2 AND status = 'posted'",
        &[&account, &date],
    )?;
    Ok(CashDayInfo {
        account_id: account,
        date: iso_of(date),
        opening,
        received: sum(&["rc"]),
        refunded: -sum(&["rf"]),
        paid_out: -sum(&["pv"]),
        other_in: sum(&["or"]),
        transfers: sum(&["tr"]),
        adjustments: sum(&["cc"]),
        closing: balance(conn, account, None, date)?,
        receipt_count,
        counts: counts_of_day(conn, account, date)?,
    })
}

/// Counts the money in an account at the end of a day (today or an earlier day): the books are what
/// was booked up to the end of that day, over/short is posted on the same day.
pub fn count(conn: &Connection, actor: &Actor, p: &CreateCashCountParams) -> Result<CashCountInfo> {
    let account = money_account(conn, "account_id", p.account_id)?;
    let date = doc_day("date", &p.date)?;
    let counted = money("counted", p.counted, true)?;
    let book = balance(conn, account, None, date)?;
    let difference = counted - book;
    let reason = optional("reason", &p.reason, 300)?;
    if difference != 0 && reason.is_none() {
        return Err(CoreError::invalid(
            "reason",
            ValidationRule::CashReasonRequired,
            "write why the count differs from the books",
        ));
    }
    let (id, number) = insert_doc(conn, actor, &NewDoc {
        kind: Some(DocKind::Cc),
        date,
        account_id: Some(account),
        amount: counted,
        reason: reason.as_deref(),
        ..Default::default()
    })?;
    post(conn, id, date, vec![dr(account, difference), cr(OVER_SHORT, difference)])?;
    audit::record(
        conn,
        actor,
        "cash.count",
        Some("fin_doc"),
        Some(&id.to_string()),
        None,
        Some(&json!({"number": number, "account": account, "book": book, "counted": counted, "difference": difference})),
    )?;
    let mut stmt = conn.prepare(&format!("{COUNT_SELECT} AND d.id = ?1"))?;
    let rows = stmt.query_map([id], map_count)?.collect::<rusqlite::Result<Vec<_>>>()?;
    named(conn, rows)?
        .into_iter()
        .next()
        .ok_or_else(|| CoreError::api(ErrorCode::Internal, "cash count not saved"))
}
