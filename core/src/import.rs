//! Patient data import/export (roadmap 3.7): a simple CSV template with a
//! fixed header, preview + per-row error report before anything is written,
//! and a matching export. Full drag-and-drop column mapping is left to a
//! later phase (see docs/releases/v0.3.0.md known limitations).

use artaveo_shared::{
    CreatePatientParams, ExportResult, ImportError, ImportPatientsParams, ImportPatientsResult,
    ImportPreviewRow,
};
use rusqlite::Connection;

use crate::audit::Actor;
use crate::clock::now_iso;
use crate::error::{CoreError, Result};
use crate::patient;

pub const TEMPLATE_HEADER: &str = "full_name,father_name,phone,secondary_phone,date_of_birth,address";

/// Splits one CSV line into fields, honouring double-quoted fields (with `""`
/// as an escaped quote) — the quoting Excel/LibreOffice produce.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    fields.push(cur);
    fields
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn col<'a>(fields: &'a [String], i: usize) -> Option<&'a str> {
    fields.get(i).map(|s| s.trim()).filter(|s| !s.is_empty())
}

pub fn import_patients(
    conn: &Connection,
    actor: &Actor,
    p: &ImportPatientsParams,
) -> Result<ImportPatientsResult> {
    let bytes = data_encoding::BASE64
        .decode(p.csv_base64.as_bytes())
        .map_err(|e| CoreError::validation(format!("invalid CSV data: {e}")))?;
    let text = String::from_utf8(bytes).map_err(|_| CoreError::validation("CSV must be UTF-8 text"))?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().unwrap_or_default().trim().trim_start_matches('\u{feff}');
    if !header.eq_ignore_ascii_case(TEMPLATE_HEADER) {
        return Err(CoreError::validation(format!("expected header `{TEMPLATE_HEADER}`, got `{header}`")));
    }

    let mut preview = Vec::new();
    let mut errors = Vec::new();
    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut total = 0u32;

    for (i, line) in lines.enumerate() {
        total += 1;
        let row_number = (i + 2) as u32; // 1 = header
        let fields = split_csv_line(line);
        let full_name = col(&fields, 0).unwrap_or_default().to_string();
        let father_name = col(&fields, 1).map(str::to_string);
        let phone = col(&fields, 2).map(str::to_string);
        let secondary_phone = col(&fields, 3).map(str::to_string);
        let date_of_birth = col(&fields, 4).map(str::to_string);
        let address = col(&fields, 5).map(str::to_string);

        let mut row_errors = Vec::new();
        if let Err(e) = patient::validate_full_name(&full_name) {
            row_errors.push(e.to_string());
        }
        if let Some(ph) = phone.as_deref() {
            if let Err(e) = patient::validate_phone("phone", ph) {
                row_errors.push(e.to_string());
            }
        }
        if let Some(d) = date_of_birth.as_deref() {
            if let Err(e) = patient::validate_date("date_of_birth", d) {
                row_errors.push(e.to_string());
            }
        }

        if row_errors.is_empty() {
            if p.commit {
                let params = CreatePatientParams {
                    full_name: full_name.clone(),
                    father_name: father_name.clone(),
                    phone: phone.clone(),
                    secondary_phone: secondary_phone.clone(),
                    date_of_birth: date_of_birth.clone(),
                    address: address.clone(),
                    registration_date: None,
                    allow_duplicate: true, // bulk import: duplicate review happens before the file is prepared
                    ..Default::default()
                };
                match patient::create_patient(conn, actor, &params) {
                    Ok(_) => imported += 1,
                    Err(e) => {
                        skipped += 1;
                        errors.push(ImportError { row_number, message: e.to_string() });
                    }
                }
            }
        } else {
            skipped += 1;
            errors.extend(row_errors.iter().map(|m| ImportError { row_number, message: m.clone() }));
        }
        preview.push(ImportPreviewRow { row_number, full_name, father_name, phone, errors: row_errors });
    }

    Ok(ImportPatientsResult { total, imported, skipped, preview, errors })
}

pub fn export_patients(conn: &Connection) -> Result<ExportResult> {
    let mut stmt = conn.prepare(
        "SELECT patient_number, full_name, father_name, phone, secondary_phone, date_of_birth, address, status
         FROM patient WHERE deleted_at IS NULL ORDER BY patient_number",
    )?;
    let rows: Vec<[String; 8]> = stmt
        .query_map([], |r| {
            Ok([
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                r.get::<_, String>(7)?,
            ])
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut csv = String::from(
        "patient_number,full_name,father_name,phone,secondary_phone,date_of_birth,address,status\r\n",
    );
    for row in rows {
        csv.push_str(&row.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(","));
        csv.push_str("\r\n");
    }
    let date = &now_iso()[..10];
    Ok(ExportResult {
        csv_base64: data_encoding::BASE64.encode(csv.as_bytes()),
        file_name: format!("patients-{date}.csv"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_fields() {
        assert_eq!(split_csv_line("a,\"b,c\",d"), vec!["a", "b,c", "d"]);
        assert_eq!(split_csv_line("x,\"say \"\"hi\"\"\",y"), vec!["x", "say \"hi\"", "y"]);
    }

    #[test]
    fn csv_field_quotes_when_needed() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("a\"b"), "\"a\"\"b\"");
    }
}
