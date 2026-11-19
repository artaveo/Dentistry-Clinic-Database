//! The document header (`fin_doc`) and the double-entry ledger (`fin_gl`).
//!
//! Every money document is written through [`insert_doc`] and posted through [`post`]: its ledger
//! rows add up to exactly 0 (+ debit, − credit), or nothing is written. A void never deletes: it
//! writes the same rows with the opposite sign, dated the void day ([`reverse`]).

use artaveo_shared::*;
use rusqlite::{params, Connection, OptionalExtension};

use crate::audit::Actor;
use crate::error::{CoreError, Result};

use super::util::*;

// ───────────────────────────── account codes ─────────────────────────────

pub(crate) const CASH: i64 = 1110;
pub(crate) const RECEIVABLE: i64 = 1310;
pub(crate) const PAYABLE: i64 = 2110;
pub(crate) const DEPOSITS: i64 = 2210;
pub(crate) const ADVANCES: i64 = 1320;
pub(crate) const WAGES_PAYABLE: i64 = 2310;
pub(crate) const SHARES_PAYABLE: i64 = 2320;
pub(crate) const TAX_WAGE: i64 = 2410;
pub(crate) const TAX_RENT: i64 = 2420;
pub(crate) const TAX_CONTRACTOR: i64 = 2430;
pub(crate) const LOANS: i64 = 2510;
pub(crate) const CAPITAL: i64 = 3110;
pub(crate) const DRAWINGS: i64 = 3120;
pub(crate) const INCOME: i64 = 4110;
pub(crate) const OTHER_INCOME: i64 = 4190;
pub(crate) const DISCOUNTS: i64 = 4910;
pub(crate) const REDUCTIONS: i64 = 4920;
pub(crate) const EXPENSES: i64 = 5000;
pub(crate) const SALARIES: i64 = 5110;
pub(crate) const DOCTOR_SHARES: i64 = 5120;
pub(crate) const MATERIALS: i64 = 5230;
pub(crate) const LAB: i64 = 5240;
pub(crate) const BAD_DEBT: i64 = 5810;
pub(crate) const OVER_SHORT: i64 = 5910;
/// Parent of the money accounts.
pub(crate) const MONEY: i64 = 1100;

// ───────────────────────────── codes ─────────────────────────────

pub(crate) fn kind_code(k: DocKind) -> &'static str {
    match k {
        DocKind::Inv => "inv",
        DocKind::Rc => "rc",
        DocKind::Al => "al",
        DocKind::Rf => "rf",
        DocKind::Cn => "cn",
        DocKind::Wo => "wo",
        DocKind::Pp => "pp",
        DocKind::Bl => "bl",
        DocKind::Pv => "pv",
        DocKind::Or => "or",
        DocKind::Tr => "tr",
        DocKind::Cc => "cc",
        DocKind::Pr => "pr",
        DocKind::Ds => "ds",
    }
}

pub(crate) fn kind_of(s: &str) -> DocKind {
    match s {
        "rc" => DocKind::Rc,
        "al" => DocKind::Al,
        "rf" => DocKind::Rf,
        "cn" => DocKind::Cn,
        "wo" => DocKind::Wo,
        "pp" => DocKind::Pp,
        "bl" => DocKind::Bl,
        "pv" => DocKind::Pv,
        "or" => DocKind::Or,
        "tr" => DocKind::Tr,
        "cc" => DocKind::Cc,
        "pr" => DocKind::Pr,
        "ds" => DocKind::Ds,
        _ => DocKind::Inv,
    }
}

pub(crate) fn prefix(k: DocKind) -> &'static str {
    match k {
        DocKind::Inv => "INV",
        DocKind::Rc => "RC",
        DocKind::Al => "AL",
        DocKind::Rf => "RF",
        DocKind::Cn => "CN",
        DocKind::Wo => "WO",
        DocKind::Pp => "PP",
        DocKind::Bl => "BL",
        DocKind::Pv => "PV",
        DocKind::Or => "OR",
        DocKind::Tr => "TR",
        DocKind::Cc => "CC",
        DocKind::Pr => "PR",
        DocKind::Ds => "DS",
    }
}

pub(crate) fn status_of(s: &str) -> DocStatus {
    match s {
        "void" => DocStatus::Void,
        "superseded" => DocStatus::Superseded,
        _ => DocStatus::Posted,
    }
}

pub(crate) fn number_of(kind: &str, year: i64, serial: i64) -> String {
    format_number(prefix(kind_of(kind)), year, serial)
}

/// SQL expression building the printed number of `fin_doc d`.
pub(crate) const NUMBER_SQL: &str =
    "upper(d.kind) || '-' || d.year || '-' || substr('000000' || d.serial, -6)";

// ───────────────────────────── the header ─────────────────────────────

/// What a new document header holds; unset fields stay NULL.
#[derive(Default)]
pub(crate) struct NewDoc<'a> {
    pub kind: Option<DocKind>,
    pub date: i64,
    pub patient_id: Option<&'a str>,
    pub supplier_id: Option<&'a str>,
    pub employee_id: Option<&'a str>,
    pub doctor_id: Option<&'a str>,
    pub account_id: Option<i64>,
    pub account2_id: Option<i64>,
    pub ref_id: Option<i64>,
    pub ext_id: Option<&'a str>,
    pub amount: i64,
    pub purpose: Option<&'a str>,
    pub name: Option<&'a str>,
    pub reference: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub note: Option<&'a str>,
}

/// Writes the header with the next number of its series; returns `(id, number)`.
pub(crate) fn insert_doc(conn: &Connection, actor: &Actor, d: &NewDoc<'_>) -> Result<(i64, String)> {
    let kind = d.kind.unwrap_or(DocKind::Inv);
    let (year, serial) = next_serial(conn, prefix(kind))?;
    let (party, party_id) = if let Some(p) = d.patient_id {
        (Some("patient"), Some(p))
    } else if let Some(s) = d.supplier_id {
        (Some("supplier"), Some(s))
    } else if let Some(e) = d.employee_id {
        (Some("employee"), Some(e))
    } else if let Some(x) = d.doctor_id {
        (Some("doctor"), Some(x))
    } else {
        (None, None)
    };
    conn.execute(
        "INSERT INTO fin_doc(kind, year, serial, doc_date, party, party_id, account_id, account2_id, ref_id, ext_id, amount,
            purpose, name, reference, reason, note, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            kind_code(kind),
            year,
            serial,
            d.date,
            party,
            party_id,
            d.account_id,
            d.account2_id,
            d.ref_id,
            d.ext_id,
            d.amount,
            d.purpose,
            d.name,
            d.reference,
            d.reason,
            d.note,
            now_secs(),
            actor.user_id
        ],
    )?;
    Ok((conn.last_insert_rowid(), format_number(prefix(kind), year, serial)))
}

/// The plain facts of a header, for the code that voids or reads any kind.
#[derive(Debug, Clone)]
pub(crate) struct DocHead {
    pub id: i64,
    pub kind: DocKind,
    pub number: String,
    pub date: i64,
    pub status: DocStatus,
    pub party_id: Option<String>,
    pub ext_id: Option<String>,
    pub amount: i64,
    pub print_count: i64,
}

pub(crate) fn head(conn: &Connection, id: i64) -> Result<DocHead> {
    conn.query_row(
        "SELECT id, kind, year, serial, doc_date, status, party_id, ext_id, amount, print_count FROM fin_doc WHERE id = ?1",
        [id],
        |r| {
            let kind: String = r.get(1)?;
            Ok(DocHead {
                id: r.get(0)?,
                number: number_of(&kind, r.get(2)?, r.get(3)?),
                kind: kind_of(&kind),
                date: r.get(4)?,
                status: status_of(&r.get::<_, String>(5)?),
                party_id: r.get(6)?,
                ext_id: r.get(7)?,
                amount: r.get(8)?,
                print_count: r.get(9)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| {
        CoreError::api(ErrorCode::NotFound, format!("finance document {id}")).on_field("id", ValidationRule::DocNotFound)
    })
}

pub(crate) fn summary(h: &DocHead) -> DocSummary {
    DocSummary {
        id: h.id,
        kind: h.kind,
        number: h.number.clone(),
        date: iso_of(h.date),
        status: h.status,
        amount: h.amount,
        print_count: h.print_count,
    }
}

// ───────────────────────────── posting ─────────────────────────────

/// One ledger row before it is written: account, signed amount (+ debit, − credit), sub-ledger party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub account: i64,
    pub amount: i64,
    pub party: Option<String>,
}

pub(crate) fn dr(account: i64, amount: i64) -> Entry {
    Entry { account, amount, party: None }
}

pub(crate) fn cr(account: i64, amount: i64) -> Entry {
    Entry { account, amount: -amount, party: None }
}

impl Entry {
    pub fn of(mut self, party: &str) -> Entry {
        self.party = Some(party.to_string());
        self
    }
}

/// Drops zero rows and merges rows on the same account and party (one row each), keeping order.
pub(crate) fn tidy(entries: Vec<Entry>) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for e in entries {
        if let Some(x) = out.iter_mut().find(|x| x.account == e.account && x.party == e.party) {
            x.amount += e.amount;
        } else {
            out.push(e);
        }
    }
    out.retain(|e| e.amount != 0);
    out
}

/// Writes the ledger rows of document `doc_id`, dated `date`. They must add up to 0.
pub(crate) fn post(conn: &Connection, doc_id: i64, date: i64, entries: Vec<Entry>) -> Result<()> {
    write_rows(conn, doc_id, date, &tidy(entries), false)
}

fn write_rows(conn: &Connection, doc_id: i64, date: i64, rows: &[Entry], rev: bool) -> Result<()> {
    let sum: i64 = rows.iter().map(|e| e.amount).sum();
    if sum != 0 {
        return Err(CoreError::Integrity(format!("posting of document {doc_id} is off by {sum}")));
    }
    let first: i64 =
        conn.query_row("SELECT COALESCE(MAX(no), 0) FROM fin_gl WHERE doc_id = ?1", [doc_id], |r| r.get(0))?;
    for (i, e) in rows.iter().enumerate() {
        conn.execute(
            "INSERT INTO fin_gl(doc_id, no, date, account_id, amount, party_id, rev) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![doc_id, first + 1 + i as i64, date, e.account, e.amount, e.party, rev as i64],
        )?;
    }
    Ok(())
}

/// The posted (not yet reversed) rows of a document.
pub(crate) fn rows_of(conn: &Connection, doc_id: i64) -> Result<Vec<Entry>> {
    let mut stmt = conn.prepare_cached(
        "SELECT account_id, amount, party_id FROM fin_gl WHERE doc_id = ?1 AND rev = 0 ORDER BY no",
    )?;
    let rows = stmt
        .query_map([doc_id], |r| Ok(Entry { account: r.get(0)?, amount: r.get(1)?, party: r.get(2)? }))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Writes the opposite of a document's posting, dated `date`.
pub(crate) fn reverse(conn: &Connection, doc_id: i64, date: i64) -> Result<()> {
    let rows: Vec<Entry> =
        rows_of(conn, doc_id)?.into_iter().map(|e| Entry { amount: -e.amount, ..e }).collect();
    if rows.is_empty() {
        return Ok(());
    }
    write_rows(conn, doc_id, date, &rows, true)
}

/// Marks a document void (with its reason) and reverses its posting today.
pub(crate) fn void_doc(conn: &Connection, actor: &Actor, h: &DocHead, reason: &str) -> Result<()> {
    if h.status != DocStatus::Posted {
        return Err(CoreError::invalid("id", ValidationRule::DocVoid, "this document is already void"));
    }
    let changed = conn.execute(
        "UPDATE fin_doc SET status = 'void', void_reason = ?1, voided_at = ?2, voided_by = ?3 WHERE id = ?4 AND status = 'posted'",
        params![reason, now_secs(), actor.user_id, h.id],
    )?;
    crate::db::expect_one_row(changed, "finance document")?;
    reverse(conn, h.id, today())
}

// ───────────────────────────── balances ─────────────────────────────

/// Debits − credits of an account (and optionally one party) up to and including `until`.
pub(crate) fn balance(conn: &Connection, account: i64, party: Option<&str>, until: i64) -> Result<i64> {
    match party {
        None => scalar(
            conn,
            "SELECT SUM(amount) FROM fin_gl WHERE account_id = ?1 AND date <= ?2",
            &[&account, &until],
        ),
        Some(p) => scalar(
            conn,
            "SELECT SUM(amount) FROM fin_gl WHERE party_id = ?1 AND account_id = ?2 AND date <= ?3",
            &[&p, &account, &until],
        ),
    }
}

// ───────────────────────────── accounts in documents ─────────────────────────────

pub(crate) struct AccountRow {
    pub id: i64,
    pub kind: String,
    pub money: Option<String>,
    pub is_active: bool,
}

pub(crate) fn account_row(conn: &Connection, id: i64) -> Result<Option<AccountRow>> {
    Ok(conn
        .query_row("SELECT id, type, money, is_active FROM fin_account WHERE id = ?1", [id], |r| {
            Ok(AccountRow {
                id: r.get(0)?,
                kind: r.get(1)?,
                money: r.get(2)?,
                is_active: r.get::<_, i64>(3)? == 1,
            })
        })
        .optional()?)
}

fn usable(conn: &Connection, field: &str, id: i64) -> Result<AccountRow> {
    let a = account_row(conn, id)?.ok_or_else(|| {
        CoreError::invalid(field, ValidationRule::AccountNotFound, format!("no account {id}"))
    })?;
    if !a.is_active {
        return Err(CoreError::invalid(
            field,
            ValidationRule::AccountInactive,
            format!("account {id} is turned off"),
        ));
    }
    Ok(a)
}

/// A money account (cash, bank, mobile) that is turned on.
pub(crate) fn money_account(conn: &Connection, field: &str, id: i64) -> Result<i64> {
    let a = usable(conn, field, id)?;
    if a.money.is_none() {
        return Err(CoreError::invalid(
            field,
            ValidationRule::AccountKind,
            format!("{id} is not a money account"),
        ));
    }
    Ok(a.id)
}

/// An expense account a person may book to by hand (not the parent 5000, not bad debts or over/short).
pub(crate) fn expense_account(conn: &Connection, field: &str, id: i64) -> Result<i64> {
    let a = usable(conn, field, id)?;
    if a.kind != "expense" || matches!(id, EXPENSES | BAD_DEBT | OVER_SHORT) {
        return Err(CoreError::invalid(
            field,
            ValidationRule::AccountKind,
            format!("{id} is not an expense account"),
        ));
    }
    Ok(a.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tidy_merges_same_account_and_drops_zeroes() {
        let rows = tidy(vec![
            dr(RECEIVABLE, 500).of("p1"),
            dr(RECEIVABLE, 300).of("p1"),
            cr(INCOME, 800),
            dr(DISCOUNTS, 0),
        ]);
        assert_eq!(rows, vec![dr(RECEIVABLE, 800).of("p1"), cr(INCOME, 800)]);
    }

    #[test]
    fn every_kind_has_its_own_prefix() {
        use DocKind::*;
        let all = [Inv, Rc, Al, Rf, Cn, Wo, Pp, Bl, Pv, Or, Tr, Cc, Pr, Ds];
        for k in all {
            assert_eq!(kind_of(kind_code(k)), k);
            assert_eq!(prefix(k).to_lowercase(), kind_code(k));
        }
    }
}
