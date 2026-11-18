//! Small helpers every finance file (and inventory, reports) uses: amounts and text checks, dates as
//! `yyyymmdd` integers, document numbers, settings.

use artaveo_shared::*;
use rusqlite::{params, Connection, OptionalExtension};

use crate::audit::Actor;
use crate::clock::{now, now_iso, today_iso};
use crate::error::{CoreError, Result};
use crate::scheduling::{format_date, parse_date};

/// Highest amount accepted: 1 000 000 000 AFN (in AFN × 100).
pub(crate) const MAX_AMOUNT: i64 = 100_000_000_000;
/// Longest list a finance read returns.
pub(crate) const MAX_LIST: u32 = 500;

pub(crate) fn money(field: &str, v: i64, allow_zero: bool) -> Result<i64> {
    let ok = if allow_zero { v >= 0 } else { v > 0 };
    if !ok || v > MAX_AMOUNT {
        return Err(CoreError::invalid(field, ValidationRule::BillingAmount, format!("{field} is not a valid amount")));
    }
    Ok(v)
}

pub(crate) fn required(field: &str, s: &str, max: usize) -> Result<String> {
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

pub(crate) fn optional(field: &str, s: &Option<String>, max: usize) -> Result<Option<String>> {
    match s.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(v) => required(field, v, max).map(Some),
    }
}

pub(crate) fn date_of(field: &'static str, s: &str) -> Result<String> {
    Ok(format_date(parse_date(field, s.trim())?))
}

pub(crate) fn opt_date(field: &'static str, s: &Option<String>) -> Result<Option<String>> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| date_of(field, s)).transpose()
}

pub(crate) fn scalar(conn: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> Result<i64> {
    Ok(conn.query_row(sql, args, |r| r.get::<_, Option<i64>>(0))?.unwrap_or(0))
}

/// Next number of a document series (`INV-1405-000001`), taken inside the writing transaction so
/// none is skipped or repeated. The year is the Shamsi year of today (ADR-08).
pub(crate) fn next_number(conn: &Connection, prefix: &str) -> Result<String> {
    let (year, serial) = next_serial(conn, prefix)?;
    Ok(format_number(prefix, year, serial))
}

pub(crate) fn next_serial(conn: &Connection, prefix: &str) -> Result<(i64, i64)> {
    let year = crate::documents::shamsi_year_today() as i64;
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
    Ok((year, n))
}

pub(crate) fn format_number(prefix: &str, year: i64, serial: i64) -> String {
    format!("{prefix}-{year}-{serial:06}")
}

pub(crate) fn parse_teeth(s: &str) -> Vec<u8> {
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

pub(crate) fn teeth_text(teeth: &[u8]) -> String {
    teeth.iter().map(u8::to_string).collect::<Vec<_>>().join(",")
}

pub(crate) fn trio(fa: String, ps: String, en: String) -> Translations {
    Translations { fa, ps, en }
}

pub(crate) fn check_patient(conn: &Connection, patient_id: &str) -> Result<()> {
    let row: Option<Option<String>> = conn
        .query_row("SELECT merged_into_id FROM patient WHERE id = ?1 AND deleted_at IS NULL", [patient_id], |r| r.get(0))
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

pub(crate) fn check_doctor(conn: &Connection, doctor_id: &Option<String>) -> Result<Option<String>> {
    match doctor_id.as_deref().filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(d) => {
            crate::scheduling::get_doctor(conn, d).map_err(|e| e.on_field("doctor_id", ValidationRule::DoctorNotFound))?;
            Ok(Some(d.to_string()))
        }
    }
}

// ───────────────────────────── dates ─────────────────────────────

/// `2026-11-18` → `20261118`.
pub(crate) fn ymd(iso: &str) -> i64 {
    iso.get(0..10).map(|d| d.replace('-', "")).and_then(|d| d.parse().ok()).unwrap_or(0)
}

/// `20261118` → `2026-11-18`.
pub(crate) fn iso_of(ymd: i64) -> String {
    format!("{:04}-{:02}-{:02}", ymd / 10_000, ymd / 100 % 100, ymd % 100)
}

pub(crate) fn today() -> i64 {
    ymd(&today_iso())
}

/// Unix seconds now (when a row was written).
pub(crate) fn now_secs() -> i64 {
    now().unix_timestamp()
}

/// Unix seconds → UTC ISO-8601.
pub(crate) fn iso_at(secs: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(secs).map(crate::clock::iso).unwrap_or_else(|_| now_iso())
}

/// A day the user picked for a document: valid, not in the future, at most a year back.
pub(crate) fn doc_day(field: &'static str, s: &str) -> Result<i64> {
    let day = date_of(field, s)?;
    let oldest = format_date(parse_date("today", &today_iso())? - time::Duration::days(365));
    if day > today_iso() {
        return Err(CoreError::invalid(field, ValidationRule::DateFormat, "the date is in the future"));
    }
    if day < oldest {
        return Err(CoreError::invalid(field, ValidationRule::DateFormat, "the date is more than a year back"));
    }
    Ok(ymd(&day))
}

/// A `from`/`to` pair (both days, inclusive) as `yyyymmdd`.
pub(crate) fn period(from: &str, to: &str) -> Result<(i64, i64)> {
    let f = ymd(&date_of("from", from)?);
    let t = ymd(&date_of("to", to)?);
    if f > t {
        return Err(CoreError::invalid("to", ValidationRule::PeriodRange, "the end is before the start"));
    }
    Ok((f, t))
}

/// Days from `a` to `b` (both `yyyymmdd`); negative when `b` is earlier.
pub(crate) fn days_between(a: i64, b: i64) -> i64 {
    match (parse_date("a", &iso_of(a)), parse_date("b", &iso_of(b))) {
        (Ok(x), Ok(y)) => (y - x).whole_days(),
        _ => 0,
    }
}

// ───────────────────────────── settings ─────────────────────────────

pub(crate) fn setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

pub(crate) fn set_setting(conn: &Connection, actor: &Actor, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO setting(key, value, updated_at, updated_by) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at, updated_by = excluded.updated_by",
        params![key, value, now_iso(), actor.user_id],
    )?;
    Ok(())
}

/// The display name of a user (for "created by").
pub(crate) fn user_name(conn: &Connection, user_id: &Option<String>) -> Result<Option<String>> {
    match user_id {
        None => Ok(None),
        Some(id) => Ok(conn
            .query_row("SELECT display_name FROM app_user WHERE id = ?1", [id], |r| r.get(0))
            .optional()?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip_as_integers() {
        assert_eq!(ymd("2026-11-18"), 20261118);
        assert_eq!(ymd("2026-11-18T10:00:00Z"), 20261118);
        assert_eq!(iso_of(20260301), "2026-03-01");
        assert_eq!(days_between(20260131, 20260302), 30);
        assert_eq!(days_between(20260302, 20260131), -30);
    }

    #[test]
    fn numbers_are_padded_to_six_digits() {
        assert_eq!(format_number("INV", 1405, 12), "INV-1405-000012");
    }
}
