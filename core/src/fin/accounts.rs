//! The chart of accounts: list, a clinic's own expense and money accounts, and one account's ledger.

use artaveo_shared::*;
use rusqlite::{params, Connection};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::error::{CoreError, Result};

use super::ledger::*;
use super::util::*;

/// Rows one account ledger returns.
const MAX_LEDGER: usize = 5000;

fn type_of(s: &str) -> AccountType {
    match s {
        "liability" => AccountType::Liability,
        "equity" => AccountType::Equity,
        "income" => AccountType::Income,
        "contra" => AccountType::Contra,
        "expense" => AccountType::Expense,
        _ => AccountType::Asset,
    }
}

pub(crate) fn money_of(s: &str) -> Option<MoneyKind> {
    match s {
        "cash" => Some(MoneyKind::Cash),
        "bank" => Some(MoneyKind::Bank),
        "mobile" => Some(MoneyKind::Mobile),
        _ => None,
    }
}

fn money_code(m: MoneyKind) -> &'static str {
    match m {
        MoneyKind::Cash => "cash",
        MoneyKind::Bank => "bank",
        MoneyKind::Mobile => "mobile",
    }
}

const ACCOUNT_SELECT: &str = "SELECT a.id, a.parent_id, a.type, a.name, a.money, a.bank_name, a.bank_no, a.is_system,
    a.is_active, a.version, (SELECT COALESCE(SUM(g.amount), 0) FROM fin_gl g WHERE g.account_id = a.id)
    FROM fin_account a";

fn map_account(r: &rusqlite::Row<'_>) -> rusqlite::Result<AccountInfo> {
    Ok(AccountInfo {
        id: r.get(0)?,
        parent_id: r.get(1)?,
        kind: type_of(&r.get::<_, String>(2)?),
        name: r.get(3)?,
        money: r.get::<_, Option<String>>(4)?.as_deref().and_then(money_of),
        bank_name: r.get(5)?,
        bank_no: r.get(6)?,
        is_system: r.get::<_, i64>(7)? == 1,
        is_active: r.get::<_, i64>(8)? == 1,
        version: r.get(9)?,
        balance: r.get(10)?,
    })
}

pub fn list(conn: &Connection, p: &AccountListParams) -> Result<Vec<AccountInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{ACCOUNT_SELECT} {} ORDER BY a.id",
        if p.include_inactive { "" } else { "WHERE a.is_active = 1" }
    ))?;
    let rows = stmt.query_map([], map_account)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> Result<AccountInfo> {
    conn.query_row(&format!("{ACCOUNT_SELECT} WHERE a.id = ?1"), [id], map_account).map_err(|_| {
        CoreError::invalid("id", ValidationRule::AccountNotFound, format!("no account {id}"))
    })
}

/// A new expense account (under 5000) or money account (under 1100), or a change of name / on-off.
pub fn save(conn: &Connection, actor: &Actor, p: &SaveAccountParams) -> Result<AccountInfo> {
    let name = optional("name", &p.name, 80)?;
    let bank_name = optional("bank_name", &p.bank_name, 80)?;
    let bank_no = optional("bank_no", &p.bank_no, 40)?;
    match p.id {
        None => {
            let parent = p.parent_id.unwrap_or(EXPENSES);
            let name = name.ok_or_else(|| CoreError::invalid("name", ValidationRule::AccountName, "the account needs a name"))?;
            let (kind, money, lo, hi) = match parent {
                EXPENSES => ("expense", None, 5300, 5799),
                MONEY => {
                    let m = p.money.ok_or_else(|| {
                        CoreError::invalid("money", ValidationRule::Required, "cash, bank or mobile money?")
                    })?;
                    match m {
                        MoneyKind::Cash => ("asset", Some(m), 1120, 1199),
                        _ => ("asset", Some(m), 1211, 1299),
                    }
                }
                _ => {
                    return Err(CoreError::invalid(
                        "parent_id",
                        ValidationRule::AccountKind,
                        "new accounts go under expenses (5000) or money (1100)",
                    ))
                }
            };
            let taken: i64 = scalar(conn, "SELECT MAX(id) FROM fin_account WHERE id BETWEEN ?1 AND ?2", &[&lo, &hi])?;
            let id = if taken == 0 { lo } else { taken + 1 };
            if id > hi {
                return Err(CoreError::invalid("parent_id", ValidationRule::AccountKind, "no free account code left"));
            }
            conn.execute(
                "INSERT INTO fin_account(id, parent_id, type, name, money, bank_name, bank_no, is_active)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![id, parent, kind, name, money.map(money_code), bank_name, bank_no, p.is_active],
            )?;
            let after = get(conn, id)?;
            audit::record(conn, actor, "account.create", Some("fin_account"), Some(&id.to_string()), None, Some(&json!(after)))?;
            Ok(after)
        }
        Some(id) => {
            let before = get(conn, id)?;
            if !p.is_active && matches!(id, CASH | RECEIVABLE | DEPOSITS | PAYABLE | INCOME) {
                return Err(CoreError::invalid("is_active", ValidationRule::AccountKind, "this account cannot be turned off"));
            }
            if !before.is_system && name.is_none() {
                return Err(CoreError::invalid("name", ValidationRule::AccountName, "the account needs a name"));
            }
            let changed = conn.execute(
                "UPDATE fin_account SET name = CASE WHEN is_system = 1 THEN name ELSE ?1 END, bank_name = ?2, bank_no = ?3,
                    is_active = ?4, version = version + 1
                 WHERE id = ?5 AND version = ?6",
                params![name, bank_name, bank_no, p.is_active, id, p.version.unwrap_or(before.version)],
            )?;
            crate::db::expect_one_row(changed, "account")?;
            let after = get(conn, id)?;
            audit::record(
                conn,
                actor,
                "account.update",
                Some("fin_account"),
                Some(&id.to_string()),
                Some(&json!(before)),
                Some(&json!(after)),
            )?;
            Ok(after)
        }
    }
}

/// One account's rows in a period (optionally one supplier's or patient's), with the running balance.
pub fn ledger(conn: &Connection, p: &AccountLedgerParams) -> Result<AccountLedger> {
    get(conn, p.account_id)?;
    let (from, to) = period(&p.from, &p.to)?;
    let party = p.party_id.as_deref().filter(|s| !s.is_empty());
    let opening = balance(conn, p.account_id, party, from - 1)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT g.date, d.id, d.kind, d.year, d.serial, g.amount, g.rev,
            COALESCE(d.name, d.reason, d.note, (SELECT name FROM fin_line l WHERE l.doc_id = d.id AND l.no = 1), '')
         FROM fin_gl g JOIN fin_doc d ON d.id = g.doc_id
         WHERE g.account_id = ?1 AND g.date BETWEEN ?2 AND ?3 {}
         ORDER BY g.date, g.doc_id, g.no LIMIT {}",
        if party.is_some() { "AND g.party_id = ?4" } else { "AND ?4 IS NULL" },
        MAX_LEDGER + 1
    ))?;
    let mut running = opening;
    let mut lines: Vec<LedgerLine> = stmt
        .query_map(params![p.account_id, from, to, party], |r| {
            let kind: String = r.get(2)?;
            Ok(LedgerLine {
                date: iso_of(r.get(0)?),
                doc_id: r.get(1)?,
                number: number_of(&kind, r.get(3)?, r.get(4)?),
                kind: kind_of(&kind),
                amount: r.get(5)?,
                reversal: r.get::<_, i64>(6)? == 1,
                description: r.get(7)?,
                balance: 0,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let truncated = lines.len() > MAX_LEDGER;
    lines.truncate(MAX_LEDGER);
    for l in &mut lines {
        running += l.amount;
        l.balance = running;
    }
    Ok(AccountLedger {
        account_id: p.account_id,
        from: iso_of(from),
        to: iso_of(to),
        opening,
        closing: balance(conn, p.account_id, party, to)?,
        lines,
        truncated,
    })
}
