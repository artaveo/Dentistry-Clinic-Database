//! Patient data import/export (roadmap 3.7, OF-017): CSV and Excel (.xlsx)
//! files with free column mapping. `inspect_import` reads a file and suggests
//! which column is which patient field from the header names (Dari, Pashto and
//! English); `import_patients` applies the user's mapping, previews every row
//! with precise per-field errors and, on `commit`, creates the valid ones.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use artaveo_shared::{
    ColumnMapping, CreatePatientParams, ExportResult, ImportError, ImportField, ImportInspectParams,
    ImportInspectResult, ImportPatientsParams, ImportPatientsResult, ImportPreviewRow, ValidationRule,
};
use calamine::{Data, Reader, Xlsx};
use rusqlite::Connection;
use time::{Date, Duration};

use crate::audit::Actor;
use crate::calendar::{digits, ShamsiDate};
use crate::clock::now_iso;
use crate::error::{CoreError, Result};
use crate::normalize::normalize_for_search;
use crate::patient;

/// Rows shown on the mapping screen.
const SAMPLE_ROWS: usize = 5;
/// A spreadsheet export of a clinic is a few thousand rows; this stops a runaway file.
const MAX_ROWS: usize = 100_000;

// ───────────────────────────── reading files ─────────────────────────────

struct Table {
    kind: &'static str,
    sheets: Vec<String>,
    sheet: Option<String>,
    /// Every row of the file, blank ones included, so row numbers match the spreadsheet / text file.
    rows: Vec<Vec<String>>,
    /// Spreadsheet row number of `rows[0]` minus one (an Excel sheet may start below row 1).
    row_offset: u32,
}

fn is_blank(row: &[String]) -> bool {
    row.iter().all(|c| c.trim().is_empty())
}

impl Table {
    /// The first non-blank row is the header.
    fn header_index(&self) -> Option<usize> {
        self.rows.iter().position(|r| !is_blank(r))
    }

    fn header(&self) -> &[String] {
        &self.rows[self.header_index().unwrap_or(0)]
    }

    /// `(spreadsheet row number, cells)` of every non-blank row under the header.
    fn data_rows(&self) -> impl Iterator<Item = (u32, &Vec<String>)> {
        let first = self.header_index().map_or(0, |h| h + 1);
        self.rows
            .iter()
            .enumerate()
            .skip(first)
            .filter(|(_, r)| !is_blank(r))
            .map(|(i, r)| (self.row_offset + i as u32 + 1, r))
    }
}

fn read_error(detail: impl Into<String>) -> CoreError {
    CoreError::invalid("file_base64", ValidationRule::ImportFileRead, detail)
}

/// Splits one CSV line into fields, honouring double-quoted fields (with `""`
/// as an escaped quote) — the quoting Excel/LibreOffice produce.
fn split_csv_line(line: &str, delimiter: char) -> Vec<String> {
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
            c if c == delimiter && !in_quotes => fields.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    fields.push(cur);
    fields
}

/// Excel in many locales writes `;` (or tabs) instead of commas.
fn detect_delimiter(header: &str) -> char {
    [',', ';', '\t'].into_iter().max_by_key(|d| header.matches(*d).count()).unwrap_or(',')
}

fn read_csv(bytes: &[u8]) -> Result<Table> {
    let text = std::str::from_utf8(bytes).map_err(|_| read_error("CSV must be UTF-8 text"))?;
    let text = text.trim_start_matches('\u{feff}');
    let delimiter = text.lines().find(|l| !l.trim().is_empty()).map(detect_delimiter).unwrap_or(',');
    let rows: Vec<Vec<String>> = text
        .lines()
        .take(MAX_ROWS + 1)
        .map(|l| split_csv_line(l, delimiter).into_iter().map(|f| f.trim().to_string()).collect())
        .collect();
    Ok(Table { kind: "csv", sheets: Vec::new(), sheet: None, rows, row_offset: 0 })
}

/// Excel stores dates as days since 1899-12-30.
fn excel_serial_to_iso(serial: f64) -> Option<String> {
    let base = Date::from_calendar_date(1899, time::Month::December, 30).ok()?;
    let d = base.checked_add(Duration::days(serial.floor() as i64))?;
    Some(format!("{:04}-{:02}-{:02}", d.year(), d.month() as u8, d.day()))
}

fn cell_text(c: &Data) -> String {
    match c {
        Data::Empty | Data::Error(_) => String::new(),
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.trim().to_string(),
        // Phone numbers and ages arrive as numbers: no ".0" tail, no exponent.
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => excel_serial_to_iso(dt.as_f64()).unwrap_or_default(),
    }
}

fn read_xlsx(bytes: &[u8], sheet: Option<&str>) -> Result<Table> {
    let mut book: Xlsx<_> = Xlsx::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| read_error(format!("not a readable .xlsx file: {e}")))?;
    let sheets = book.sheet_names();
    let chosen = match sheet.filter(|s| !s.is_empty()) {
        Some(s) if sheets.iter().any(|n| n == s) => s.to_string(),
        Some(s) => return Err(read_error(format!("sheet `{s}` not found"))),
        None => sheets.first().cloned().ok_or_else(|| read_error("the workbook has no sheets"))?,
    };
    let range = book.worksheet_range(&chosen).map_err(|e| read_error(format!("cannot read sheet: {e}")))?;
    let rows: Vec<Vec<String>> =
        range.rows().take(MAX_ROWS + 1).map(|r| r.iter().map(cell_text).collect::<Vec<_>>()).collect();
    let row_offset = range.start().map_or(0, |(row, _)| row);
    Ok(Table { kind: "xlsx", sheets, sheet: Some(chosen), rows, row_offset })
}

fn read_table(file_base64: &str, file_name: Option<&str>, sheet: Option<&str>) -> Result<Table> {
    let bytes = data_encoding::BASE64
        .decode(file_base64.as_bytes())
        .map_err(|e| read_error(format!("invalid file data: {e}")))?;
    if bytes.is_empty() {
        return Err(read_error("the file is empty"));
    }
    let ext = file_name.and_then(|n| n.rsplit('.').next()).map(str::to_ascii_lowercase);
    let table = if bytes.starts_with(b"PK") {
        read_xlsx(&bytes, sheet)?
    } else if matches!(ext.as_deref(), Some("xls" | "xlsm" | "xlsb" | "ods"))
        || bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0])
    {
        return Err(CoreError::invalid(
            "file_base64",
            ValidationRule::ImportFileType,
            "only .xlsx and .csv files are supported (save the old Excel file as .xlsx)",
        ));
    } else {
        read_csv(&bytes)?
    };
    if table.header_index().is_none() {
        return Err(read_error("the file has no rows"));
    }
    Ok(table)
}

// ───────────────────────────── header → field guess ─────────────────────────────

/// Letters and digits only, search-normalized: "Father's Name", "نام_پدر" and "نام پدر" all fold alike.
fn header_key(s: &str) -> String {
    normalize_for_search(s).chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Header names people really use, per field (Dari, Pashto, English).
const SYNONYMS: &[(ImportField, &[&str])] = &[
    (
        ImportField::FullName,
        &[
            "fullname",
            "name",
            "patientname",
            "patient",
            "نام",
            "نامکامل",
            "نامبیمار",
            "بیمار",
            "نامتخلص",
            "ناموتخلص",
            "نومتخلص",
            "نوم",
            "بشپړنوم",
            "دناروغنوم",
            "ناروغ",
        ],
    ),
    (
        ImportField::FatherName,
        &["fathername", "father", "fathersname", "sonof", "ولد", "نامپدر", "پدر", "اسمپدر", "دپلارنوم", "پلار"],
    ),
    (
        ImportField::Phone,
        &[
            "phone",
            "phonenumber",
            "tel",
            "telephone",
            "mobile",
            "mobilenumber",
            "cell",
            "contact",
            "شماره",
            "شمارهتماس",
            "تماس",
            "شمارهتلفن",
            "تلفن",
            "موبایل",
            "موبائل",
            "شمارهموبایل",
            "تلیفون",
            "دتلیفونشمېره",
            "شمېره",
            "موبایلشمېره",
        ],
    ),
    (
        ImportField::SecondaryPhone,
        &[
            "secondaryphone",
            "phone2",
            "mobile2",
            "altphone",
            "alternatephone",
            "secondarymobile",
            "شمارهدوم",
            "تلفندوم",
            "شمارهتماسدوم",
            "دوهمتلیفون",
        ],
    ),
    (
        ImportField::DateOfBirth,
        &["dateofbirth", "dob", "birthdate", "birthday", "تاریختولد", "تولد", "دزېږیدنېنیټه", "زېږدنېنیټه"],
    ),
    (ImportField::ApproximateAge, &["age", "approximateage", "سن", "عمر", "عمرتقریبی"]),
    (ImportField::Gender, &["gender", "sex", "جنسیت", "جنس"]),
    (ImportField::Province, &["province", "provincename", "wilayat", "ولایت"]),
    (ImportField::Address, &["address", "آدرس", "ادرس", "نشانی", "پته"]),
    (
        ImportField::EmergencyContactName,
        &["emergencycontact", "emergencycontactname", "emergencyname", "تماساضطراری", "نامتماساضطراری"],
    ),
    (
        ImportField::EmergencyContactPhone,
        &["emergencycontactphone", "emergencyphone", "شمارهتماساضطراری", "تلفناضطراری"],
    ),
    (
        ImportField::Notes,
        &[
            "notes",
            "note",
            "remarks",
            "comment",
            "comments",
            "یادداشت",
            "توضیحات",
            "ملاحظات",
            "یادښت",
            "تبصره",
        ],
    ),
    (
        ImportField::RegistrationDate,
        &[
            "registrationdate",
            "registered",
            "regdate",
            "dateregistered",
            "تاریخثبت",
            "تاریخراجستر",
            "دثبتنیټه",
        ],
    ),
];

/// Best field for each header: an exact name beats a name that merely contains a known
/// word, and a longer word beats a shorter one ("father name" is the father, not the patient).
/// Each field and each column is used at most once.
pub fn suggest_mapping(headers: &[String]) -> Vec<ColumnMapping> {
    let mut scored: Vec<(usize, u32, ImportField, u32)> = Vec::new();
    for (column, header) in headers.iter().enumerate() {
        let key = header_key(header);
        if key.is_empty() {
            continue;
        }
        for (field, words) in SYNONYMS {
            let best = words
                .iter()
                .filter_map(|w| {
                    let w = header_key(w);
                    let len = w.chars().count() as u32;
                    if key == w {
                        Some(1000 + len)
                    } else if len >= 4 && key.contains(&w) {
                        Some(len)
                    } else {
                        None
                    }
                })
                .max();
            if let Some(score) = best {
                scored.push((column, score, *field, score));
            }
        }
    }
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut used_columns = HashSet::new();
    let mut used_fields = HashSet::new();
    let mut out = Vec::new();
    for (column, _, field, _) in scored {
        if used_columns.contains(&column) || used_fields.contains(&field) {
            continue;
        }
        used_columns.insert(column);
        used_fields.insert(field);
        out.push(ColumnMapping { column: column as u32, field });
    }
    out.sort_by_key(|m| m.column);
    out
}

pub fn inspect_import(p: &ImportInspectParams) -> Result<ImportInspectResult> {
    let table = read_table(&p.file_base64, Some(&p.file_name), p.sheet.as_deref())?;
    let headers = table.header().to_vec();
    Ok(ImportInspectResult {
        file_kind: table.kind.into(),
        sheets: table.sheets.clone(),
        sheet: table.sheet.clone(),
        suggested: suggest_mapping(&headers),
        sample_rows: table.data_rows().take(SAMPLE_ROWS).map(|(_, r)| r.clone()).collect(),
        total_rows: table.data_rows().count() as u32,
        headers,
    })
}

// ───────────────────────────── cell values ─────────────────────────────

/// ISO date from what people type or Excel exports: `2024-05-20`, `2024/5/20`, Persian digits,
/// or a Solar Hijri date (year below 1700, e.g. `1403/02/31`).
fn parse_date_cell(text: &str) -> Option<String> {
    let t = digits::to_latin(text.trim());
    let t = t.split([' ', 'T']).next().unwrap_or("");
    let parts: Vec<&str> = t.split(['-', '/', '.']).collect();
    let [y, m, d] = parts[..] else { return None };
    let (y, m, d): (i32, u8, u8) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    if y < 1700 {
        let g = ShamsiDate::new(y, m, d).ok()?.to_gregorian();
        return Some(format!("{:04}-{:02}-{:02}", g.year, g.month, g.day));
    }
    let date = Date::from_calendar_date(y, time::Month::try_from(m).ok()?, d).ok()?;
    Some(format!("{:04}-{:02}-{:02}", date.year(), date.month() as u8, date.day()))
}

/// Lookup tables from any-language label to id, built once per import.
struct Lookups {
    gender: HashMap<String, String>,
    province: HashMap<String, String>,
}

impl Lookups {
    fn load(conn: &Connection) -> Result<Self> {
        let mut gender = HashMap::new();
        let mut stmt = conn.prepare(
            "SELECT i.id, t.label FROM reference_item i
             JOIN reference_type ty ON ty.id = i.type_id AND ty.code = 'gender'
             JOIN reference_translation t ON t.reference_item_id = i.id",
        )?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (id, label) = row?;
            gender.insert(header_key(&label), id);
        }
        // Single-letter abbreviations used in many exports.
        for (abbr, code) in [("m", "male"), ("f", "female")] {
            let id = crate::ids::seed_id("reference_item", &format!("gender/{code}"));
            gender.entry(abbr.to_string()).or_insert(id);
        }
        let mut province = HashMap::new();
        let mut stmt =
            conn.prepare("SELECT entity_id, label FROM geo_translation WHERE entity_type = 'province'")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (id, label) = row?;
            province.insert(header_key(&label), id);
        }
        Ok(Lookups { gender, province })
    }
}

fn row_error(
    row_number: u32,
    field: ImportField,
    rule: ValidationRule,
    message: impl Into<String>,
) -> ImportError {
    ImportError { row_number, message: message.into(), field: Some(field), rule: Some(rule) }
}

/// Maps a validation error of the patient rules back to the import field it is about.
fn field_of(name: Option<&str>) -> Option<ImportField> {
    Some(match name? {
        "full_name" => ImportField::FullName,
        "phone" => ImportField::Phone,
        "secondary_phone" => ImportField::SecondaryPhone,
        "date_of_birth" => ImportField::DateOfBirth,
        "approximate_age" => ImportField::ApproximateAge,
        "emergency_contact_phone" => ImportField::EmergencyContactPhone,
        _ => return None,
    })
}

fn core_error(row_number: u32, e: &CoreError) -> ImportError {
    let detail = match e {
        CoreError::Api { detail, .. } => detail.clone(),
        other => other.to_string(),
    };
    let rule = match e {
        CoreError::Api { rule, .. } => *rule,
        _ => None,
    };
    ImportError { row_number, message: detail, field: field_of(e.field()), rule }
}

/// One file row turned into patient parameters, or the reasons it cannot be imported.
fn build_row(
    row_number: u32,
    cells: &[String],
    mapping: &[ColumnMapping],
    lookups: &Lookups,
) -> (CreatePatientParams, Vec<ImportError>) {
    let mut p = CreatePatientParams { allow_duplicate: true, ..Default::default() };
    let mut errors = Vec::new();
    for m in mapping {
        let Some(text) = cells.get(m.column as usize).map(|s| s.trim()).filter(|s| !s.is_empty()) else {
            continue;
        };
        let text = text.to_string();
        match m.field {
            ImportField::FullName => p.full_name = text,
            ImportField::FatherName => p.father_name = Some(text),
            ImportField::Phone => p.phone = Some(text),
            ImportField::SecondaryPhone => p.secondary_phone = Some(text),
            ImportField::Address => p.address = Some(text),
            ImportField::EmergencyContactName => p.emergency_contact_name = Some(text),
            ImportField::EmergencyContactPhone => p.emergency_contact_phone = Some(text),
            ImportField::Notes => p.notes = Some(text),
            ImportField::DateOfBirth => match parse_date_cell(&text) {
                Some(d) => p.date_of_birth = Some(d),
                None => errors.push(row_error(
                    row_number,
                    m.field,
                    ValidationRule::DateFormat,
                    format!("`{text}` is not a date"),
                )),
            },
            ImportField::RegistrationDate => match parse_date_cell(&text) {
                Some(d) => p.registration_date = Some(d),
                None => errors.push(row_error(
                    row_number,
                    m.field,
                    ValidationRule::DateFormat,
                    format!("`{text}` is not a date"),
                )),
            },
            ImportField::ApproximateAge => match digits::to_latin(&text).trim().parse::<i64>() {
                Ok(a) => p.approximate_age = Some(a),
                Err(_) => errors.push(row_error(
                    row_number,
                    m.field,
                    ValidationRule::AgeRange,
                    format!("`{text}` is not an age"),
                )),
            },
            ImportField::Gender => match lookups.gender.get(&header_key(&text)) {
                Some(id) => p.gender_id = Some(id.clone()),
                None => errors.push(row_error(
                    row_number,
                    m.field,
                    ValidationRule::ImportColumn,
                    format!("unknown gender `{text}`"),
                )),
            },
            ImportField::Province => match lookups.province.get(&header_key(&text)) {
                Some(id) => p.province_id = Some(id.clone()),
                None => errors.push(row_error(
                    row_number,
                    m.field,
                    ValidationRule::ImportColumn,
                    format!("unknown province `{text}`"),
                )),
            },
        }
    }
    (p, errors)
}

/// Same rules as the patient form, so a bad row is reported before anything is written.
fn validate_row(row_number: u32, p: &CreatePatientParams) -> Vec<ImportError> {
    let mut errors = Vec::new();
    let mut check = |r: Result<()>| {
        if let Err(e) = r {
            errors.push(core_error(row_number, &e));
        }
    };
    check(patient::validate_full_name(&p.full_name));
    for (field, v) in [
        ("phone", &p.phone),
        ("secondary_phone", &p.secondary_phone),
        ("emergency_contact_phone", &p.emergency_contact_phone),
    ] {
        if let Some(v) = v {
            check(patient::validate_phone(field, v));
        }
    }
    if let Some(a) = p.approximate_age {
        if !(0..=120).contains(&a) {
            check(Err(CoreError::invalid(
                "approximate_age",
                ValidationRule::AgeRange,
                "approximate age must be 0-120",
            )));
        }
    }
    errors
}

pub fn import_patients(
    conn: &Connection,
    actor: &Actor,
    p: &ImportPatientsParams,
) -> Result<ImportPatientsResult> {
    let table = read_table(&p.file_base64, p.file_name.as_deref(), p.sheet.as_deref())?;
    let mapping = match &p.mapping {
        Some(m) => m.clone(),
        None => suggest_mapping(table.header()),
    };
    let columns = table.rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut seen_fields = HashSet::new();
    for m in &mapping {
        if m.column as usize >= columns || !seen_fields.insert(m.field) {
            return Err(CoreError::invalid(
                "mapping",
                ValidationRule::ImportColumn,
                "mapping names a missing column or maps one field twice",
            ));
        }
    }
    if !mapping.iter().any(|m| m.field == ImportField::FullName) {
        return Err(CoreError::invalid(
            "mapping",
            ValidationRule::ImportNoNameColumn,
            "one column must be mapped to the patient's name",
        ));
    }
    let lookups = Lookups::load(conn)?;

    let mut preview = Vec::new();
    let mut errors = Vec::new();
    let (mut imported, mut skipped, mut total) = (0u32, 0u32, 0u32);

    for (row_number, cells) in table.data_rows() {
        total += 1;
        let (params, mut row_errors) = build_row(row_number, cells, &mapping, &lookups);
        row_errors.extend(validate_row(row_number, &params));
        if row_errors.is_empty() {
            if p.commit {
                match patient::create_patient(conn, actor, &params) {
                    Ok(_) => imported += 1,
                    Err(e) => {
                        skipped += 1;
                        errors.push(core_error(row_number, &e));
                    }
                }
            }
        } else {
            skipped += 1;
            errors.extend(row_errors.iter().cloned());
        }
        preview.push(ImportPreviewRow {
            row_number,
            full_name: params.full_name,
            father_name: params.father_name,
            phone: params.phone,
            errors: row_errors,
        });
    }
    Ok(ImportPatientsResult { total, imported, skipped, preview, errors })
}

// ───────────────────────────── export ─────────────────────────────

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
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

    fn h(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    fn field_of_column(m: &[ColumnMapping], column: u32) -> Option<ImportField> {
        m.iter().find(|x| x.column == column).map(|x| x.field)
    }

    #[test]
    fn splits_quoted_fields_and_other_delimiters() {
        assert_eq!(split_csv_line("a,\"b,c\",d", ','), vec!["a", "b,c", "d"]);
        assert_eq!(split_csv_line("x,\"say \"\"hi\"\"\",y", ','), vec!["x", "say \"hi\"", "y"]);
        assert_eq!(split_csv_line("a;b;c", ';'), vec!["a", "b", "c"]);
        assert_eq!(detect_delimiter("name;phone;age"), ';');
        assert_eq!(detect_delimiter("name,phone,age"), ',');
        assert_eq!(detect_delimiter("name\tphone\tage"), '\t');
    }

    #[test]
    fn csv_field_quotes_when_needed() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn headers_are_guessed_in_three_languages() {
        let m = suggest_mapping(&h(&["Full Name", "Father's Name", "Mobile", "Age", "Gender", "Province"]));
        assert_eq!(field_of_column(&m, 0), Some(ImportField::FullName));
        assert_eq!(
            field_of_column(&m, 1),
            Some(ImportField::FatherName),
            "father name is not the patient name"
        );
        assert_eq!(field_of_column(&m, 2), Some(ImportField::Phone));
        assert_eq!(field_of_column(&m, 3), Some(ImportField::ApproximateAge));
        assert_eq!(field_of_column(&m, 4), Some(ImportField::Gender));
        assert_eq!(field_of_column(&m, 5), Some(ImportField::Province));

        let m = suggest_mapping(&h(&["نام و تخلص", "نام پدر", "شماره تماس", "تاریخ تولد", "ولایت", "آدرس"]));
        assert_eq!(field_of_column(&m, 0), Some(ImportField::FullName));
        assert_eq!(field_of_column(&m, 1), Some(ImportField::FatherName));
        assert_eq!(field_of_column(&m, 2), Some(ImportField::Phone));
        assert_eq!(field_of_column(&m, 3), Some(ImportField::DateOfBirth));
        assert_eq!(field_of_column(&m, 4), Some(ImportField::Province));
        assert_eq!(field_of_column(&m, 5), Some(ImportField::Address));

        let m = suggest_mapping(&h(&["د ناروغ نوم", "د پلار نوم", "تلیفون"]));
        assert_eq!(field_of_column(&m, 0), Some(ImportField::FullName));
        assert_eq!(field_of_column(&m, 1), Some(ImportField::FatherName));
        assert_eq!(field_of_column(&m, 2), Some(ImportField::Phone));

        // Unknown columns are left unmapped; a field is never mapped twice.
        let m = suggest_mapping(&h(&["Name", "Name", "Favourite colour"]));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn dates_accept_gregorian_shamsi_and_persian_digits() {
        assert_eq!(parse_date_cell("2024-05-20").as_deref(), Some("2024-05-20"));
        assert_eq!(parse_date_cell("2024/5/2").as_deref(), Some("2024-05-02"));
        assert_eq!(parse_date_cell("۲۰۲۴-۰۵-۲۰").as_deref(), Some("2024-05-20"));
        // 1 Hamal 1403 = 20 March 2024.
        assert_eq!(parse_date_cell("1403/01/01").as_deref(), Some("2024-03-20"));
        assert_eq!(parse_date_cell("2024-02-30"), None);
        assert_eq!(parse_date_cell("yesterday"), None);
        assert_eq!(excel_serial_to_iso(45000.0).as_deref(), Some("2023-03-15"));
    }
}
