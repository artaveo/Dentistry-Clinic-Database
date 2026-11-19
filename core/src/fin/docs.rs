//! What every finance document shares: voiding (with a reversing posting) and printing.

use artaveo_shared::*;
use rusqlite::{params, Connection};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::error::{CoreError, Result};

use super::ledger::*;
use super::util::*;

/// Voids a document of any kind with its reason; the posting is reversed today.
pub fn void(conn: &Connection, actor: &Actor, p: &VoidDocParams) -> Result<DocSummary> {
    let h = head(conn, p.id)?;
    let reason = required("reason", &p.reason, 300)?;
    if h.status != DocStatus::Posted {
        return Err(CoreError::invalid("id", ValidationRule::DocVoid, "this document is already void"));
    }
    match h.kind {
        DocKind::Inv => super::invoices::void(conn, actor, &h, &reason)?,
        DocKind::Bl if h.ext_id.is_some() => {
            return Err(CoreError::invalid(
                "id",
                ValidationRule::DocNotVoidable,
                "this bill comes from a purchase or lab case: void that instead",
            ))
        }
        _ => void_doc(conn, actor, &h, &reason)?,
    }
    let party: Option<String> =
        conn.query_row("SELECT party FROM fin_doc WHERE id = ?1", [h.id], |r| r.get(0))?;
    if matches!(h.kind, DocKind::Rc | DocKind::Al | DocKind::Rf | DocKind::Cn | DocKind::Wo) {
        if let Some(patient) = &h.party_id {
            super::receipts::check_books(conn, patient)?;
        }
    }
    if matches!(h.kind, DocKind::Pr | DocKind::Ds | DocKind::Pv) {
        super::staff::check_staff_books(conn)?;
    }
    audit::record(
        conn,
        actor,
        "fin.void",
        Some(match party.as_deref() {
            Some("patient") => "patient",
            Some("supplier") => "supplier",
            Some("employee") => "employee",
            Some("doctor") => "doctor",
            _ => "fin_doc",
        }),
        Some(&h.party_id.clone().unwrap_or_else(|| h.id.to_string())),
        Some(&json!({"number": h.number, "amount": h.amount})),
        Some(&json!({"reason": reason})),
    )?;
    Ok(summary(&head(conn, h.id)?))
}

/// A payroll, a share statement, or a salary / advance / share payment: voiding it needs the payroll
/// permission as well.
pub fn is_staff_doc(conn: &Connection, id: i64) -> Result<bool> {
    Ok(scalar(
        conn,
        "SELECT COUNT(*) FROM fin_doc WHERE id = ?1 AND (kind IN ('pr', 'ds') OR party IN ('employee', 'doctor'))",
        &[&id],
    )? > 0)
}

/// Counts a print (or a PDF) of a document; the copy number goes on the paper.
pub fn printed(conn: &Connection, actor: &Actor, p: &DocPrintedParams) -> Result<DocSummary> {
    let h = head(conn, p.id)?;
    conn.execute("UPDATE fin_doc SET print_count = print_count + 1 WHERE id = ?1", params![h.id])?;
    audit::record(
        conn,
        actor,
        if p.pdf { "fin.pdf" } else { "fin.print" },
        Some("fin_doc"),
        Some(&h.id.to_string()),
        None,
        Some(&json!({"number": h.number, "copy": h.print_count + 1})),
    )?;
    Ok(summary(&head(conn, h.id)?))
}

/// Used by tests and the health check: every document's posting adds up to 0.
pub(crate) fn unbalanced_docs(conn: &Connection) -> Result<i64> {
    scalar(
        conn,
        "SELECT COUNT(*) FROM (SELECT doc_id FROM fin_gl GROUP BY doc_id HAVING SUM(amount) <> 0)",
        &[],
    )
}
