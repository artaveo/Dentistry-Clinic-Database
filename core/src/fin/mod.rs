//! Finance (docs/design/finance-standard.md, docs/design/finance-progress.md): a double-entry ledger
//! behind every money document. Money is AFN × 100 (ADR-06); an issued document is never edited
//! (ADR-11): a mistake is a void (a reversing posting) or a new document, each with a reason.

pub mod accounts;
pub mod cash;
pub mod docs;
pub mod invoices;
pub(crate) mod ledger;
pub mod overview;
pub mod patients;
pub mod receipts;
pub mod shares;
pub mod spend;
pub mod staff;
pub(crate) mod util;

pub(crate) use util::{
    check_patient, date_of, money, next_number, opt_date, optional, parse_teeth, required, scalar, trio,
};

use rusqlite::{params, Connection};

use crate::error::Result;

/// A merged patient's money follows the kept record (documents and their sub-ledger rows).
pub(crate) fn merge_patient(conn: &Connection, keep_id: &str, merge_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE fin_doc SET party_id = ?1 WHERE party = 'patient' AND party_id = ?2",
        params![keep_id, merge_id],
    )?;
    conn.execute("UPDATE fin_gl SET party_id = ?1 WHERE party_id = ?2", params![keep_id, merge_id])?;
    Ok(())
}

/// Whether the patient has any money document (then the record is archived, never deleted).
pub(crate) fn patient_has_money(conn: &Connection, patient_id: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM fin_doc WHERE party = 'patient' AND party_id = ?1)",
        [patient_id],
        |r| r.get(0),
    )?)
}
