//! Clinic profile (roadmap 2.5): identity, geography, calendar, branding and
//! working-hours preferences set by the Setup Wizard and editable afterwards
//! from Settings. Follows the same `setting` key/value table and audited
//! get/update shape as `settings.rs`. `name`, `default_language`, `logo_path`
//! and `install_mode` live in `db_meta` / are fixed at Setup and are not
//! changed by `update` (see [`update`] doc).

use artaveo_shared::{
    CalendarSystem, ClinicMode, ClinicProfile, DayHours, InstallMode, ThemePreference, ValidationRule,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::clock::now_iso;
use crate::error::{CoreError, Result};

fn read(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

fn write(conn: &Connection, key: &str, value: &str, updated_by: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT INTO setting(key, value, updated_at, updated_by) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at, updated_by = excluded.updated_by",
        params![key, value, now_iso(), updated_by],
    )?;
    Ok(())
}

fn meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT value FROM db_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

/// Reads the clinic profile, filling in defaults for anything never set
/// (a freshly migrated database that predates this module, or a field the
/// wizard left blank).
pub fn get(conn: &Connection) -> Result<ClinicProfile> {
    let working_hours: Vec<DayHours> =
        read(conn, "clinic.working_hours")?.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
    Ok(ClinicProfile {
        name: meta(conn, "clinic_name")?.unwrap_or_default(),
        default_language: match meta(conn, "default_language")?.as_deref() {
            Some("ps") => artaveo_shared::Language::Ps,
            Some("en") => artaveo_shared::Language::En,
            _ => artaveo_shared::Language::Fa,
        },
        logo_path: read(conn, "clinic.logo_path")?,
        province_id: read(conn, "clinic.province_id")?,
        district_id: read(conn, "clinic.district_id")?,
        address: read(conn, "clinic.address")?,
        phone: read(conn, "clinic.phone")?,
        calendar_system: match read(conn, "clinic.calendar_system")?.as_deref() {
            Some("gregorian") => CalendarSystem::Gregorian,
            _ => CalendarSystem::Shamsi,
        },
        clinic_mode: match read(conn, "clinic.clinic_mode")?.as_deref() {
            Some("multi") => ClinicMode::Multi,
            _ => ClinicMode::Solo,
        },
        install_mode: match read(conn, "clinic.install_mode")?.as_deref() {
            Some("server") => InstallMode::Server,
            Some("client") => InstallMode::Client,
            _ => InstallMode::Single,
        },
        theme: match read(conn, "clinic.theme")?.as_deref() {
            Some("light") => ThemePreference::Light,
            Some("dark") => ThemePreference::Dark,
            _ => ThemePreference::System,
        },
        color_primary: read(conn, "clinic.color_primary")?.unwrap_or_else(|| "#0e7490".into()),
        color_secondary: read(conn, "clinic.color_secondary")?.unwrap_or_else(|| "#64748b".into()),
        color_accent: read(conn, "clinic.color_accent")?.unwrap_or_else(|| "#f59e0b".into()),
        working_hours,
        trial_started_at: read(conn, "clinic.trial_started_at")?,
    })
}

fn validate_color(field: &str, c: &str) -> Result<()> {
    let ok = c.len() == 7 && c.starts_with('#') && c.chars().skip(1).all(|ch| ch.is_ascii_hexdigit());
    if !ok {
        return Err(CoreError::invalid(
            field,
            ValidationRule::ColorFormat,
            format!("`{c}` is not a #rrggbb color"),
        ));
    }
    Ok(())
}

fn validate_hhmm(s: &str) -> Result<()> {
    let bad = || {
        CoreError::invalid("working_hours", ValidationRule::TimeFormat, format!("`{s}` is not an HH:MM time"))
    };
    let (h, m) = s.split_once(':').ok_or_else(bad)?;
    let h: u32 = h.parse().map_err(|_| bad())?;
    let m: u32 = m.parse().map_err(|_| bad())?;
    if h > 23 || m > 59 {
        return Err(bad());
    }
    Ok(())
}

fn validate(p: &ClinicProfile) -> Result<()> {
    validate_color("color_primary", &p.color_primary)?;
    validate_color("color_secondary", &p.color_secondary)?;
    validate_color("color_accent", &p.color_accent)?;
    let hours_error =
        |detail: &str| CoreError::invalid("working_hours", ValidationRule::WorkingHours, detail);
    if p.working_hours.len() > 7 {
        return Err(hours_error("at most 7 working-hours rows"));
    }
    for d in &p.working_hours {
        if d.day > 6 {
            return Err(hours_error("day must be 0–6 (Saturday–Friday)"));
        }
        if !d.closed {
            match (&d.open, &d.close) {
                (Some(o), Some(c)) => {
                    validate_hhmm(o)?;
                    validate_hhmm(c)?;
                    if minutes(o) >= minutes(c) {
                        return Err(hours_error("closing time must be after opening time"));
                    }
                }
                _ => return Err(hours_error("an open day needs open and close times")),
            }
        }
    }
    Ok(())
}

/// Minutes since midnight of an already-validated "HH:MM".
fn minutes(s: &str) -> u32 {
    let (h, m) = s.split_once(':').unwrap_or(("0", "0"));
    h.parse::<u32>().unwrap_or(0) * 60 + m.parse::<u32>().unwrap_or(0)
}

/// The stored logo as a `data:` URL for the header/login (roadmap 2.1b).
/// A missing or unreadable file is "no logo", never an error: the UI falls
/// back to the clinic's initial.
pub fn logo_data_url(conn: &Connection) -> Result<Option<String>> {
    let Some(path) = read(conn, "clinic.logo_path")?.filter(|p| !p.is_empty()) else { return Ok(None) };
    let mime = match std::path::Path::new(&path).extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => return Ok(None),
    };
    Ok(std::fs::read(&path)
        .ok()
        .map(|bytes| format!("data:{mime};base64,{}", data_encoding::BASE64.encode(&bytes))))
}

/// Stores a new logo (or removes it) and audits the change. `logo` is the
/// already-validated extension and bytes (`api::decode_logo`).
pub fn set_logo(
    conn: &Connection,
    actor: &Actor,
    dir: &std::path::Path,
    logo: Option<(&str, Vec<u8>)>,
) -> Result<Option<String>> {
    let before = read(conn, "clinic.logo_path")?.filter(|p| !p.is_empty());
    let path = match logo {
        Some((ext, bytes)) => {
            std::fs::create_dir_all(dir)?;
            // A new name per upload: the old file is only removed once the
            // setting points at the new one.
            let file = dir.join(format!("logo-{}.{ext}", crate::ids::new_id()));
            std::fs::write(&file, bytes)?;
            Some(file.display().to_string())
        }
        None => None,
    };
    write(conn, "clinic.logo_path", path.as_deref().unwrap_or_default(), actor.user_id.as_deref())?;
    if let Some(old) = before.as_ref().filter(|old| Some(*old) != path.as_ref()) {
        let _ = std::fs::remove_file(old);
    }
    audit::record(
        conn,
        actor,
        "clinic.set_logo",
        Some("clinic"),
        None,
        Some(&json!({ "logo": before.is_some() })),
        Some(&json!({ "logo": path.is_some() })),
    )?;
    logo_data_url(conn)
}

/// Updates the editable part of the clinic profile. `name`, `default_language`,
/// `logo_path` and `install_mode` are ignored: those are set once at Setup and
/// changed through dedicated flows in later phases (rename/branding
/// management, Phase 7 LAN conversion), never by this generic update.
pub fn update(conn: &Connection, actor: &Actor, new: &ClinicProfile) -> Result<ClinicProfile> {
    validate(new)?;
    let before = get(conn)?;
    let hours = serde_json::to_string(&new.working_hours)?;
    for (k, v) in [
        ("clinic.province_id", new.province_id.clone().unwrap_or_default()),
        ("clinic.district_id", new.district_id.clone().unwrap_or_default()),
        ("clinic.address", new.address.clone().unwrap_or_default()),
        ("clinic.phone", new.phone.clone().unwrap_or_default()),
        ("clinic.calendar_system", calendar_code(new.calendar_system).to_string()),
        ("clinic.clinic_mode", clinic_mode_code(new.clinic_mode).to_string()),
        ("clinic.theme", theme_code(new.theme).to_string()),
        ("clinic.color_primary", new.color_primary.clone()),
        ("clinic.color_secondary", new.color_secondary.clone()),
        ("clinic.color_accent", new.color_accent.clone()),
        ("clinic.working_hours", hours),
    ] {
        write(conn, k, &v, actor.user_id.as_deref())?;
    }
    let after = get(conn)?;
    audit::record(
        conn,
        actor,
        "clinic.update",
        Some("clinic"),
        None,
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

pub fn calendar_code(c: CalendarSystem) -> &'static str {
    match c {
        CalendarSystem::Shamsi => "shamsi",
        CalendarSystem::Gregorian => "gregorian",
    }
}
pub fn clinic_mode_code(c: ClinicMode) -> &'static str {
    match c {
        ClinicMode::Solo => "solo",
        ClinicMode::Multi => "multi",
    }
}
pub fn install_mode_code(c: InstallMode) -> &'static str {
    match c {
        InstallMode::Single => "single",
        InstallMode::Server => "server",
        InstallMode::Client => "client",
    }
}
pub fn theme_code(c: ThemePreference) -> &'static str {
    match c {
        ThemePreference::Light => "light",
        ThemePreference::Dark => "dark",
        ThemePreference::System => "system",
    }
}

/// Writes everything the Setup Wizard collected, in the same transaction as
/// clinic/owner creation (called from `api::setup`). `conn` is a transaction.
pub fn init_from_setup(
    conn: &Connection,
    p: &artaveo_shared::SetupParams,
    logo_path: Option<&str>,
) -> Result<()> {
    let now = now_iso();
    for (k, v) in [
        ("clinic.install_mode", install_mode_code(p.install_mode).to_string()),
        ("clinic.province_id", p.province_id.clone().unwrap_or_default()),
        ("clinic.district_id", p.district_id.clone().unwrap_or_default()),
        ("clinic.address", p.address.clone().unwrap_or_default()),
        ("clinic.phone", p.phone.clone().unwrap_or_default()),
        ("clinic.logo_path", logo_path.unwrap_or_default().to_string()),
        ("clinic.calendar_system", calendar_code(p.calendar_system).to_string()),
        ("clinic.clinic_mode", clinic_mode_code(p.clinic_mode).to_string()),
        ("clinic.theme", theme_code(p.theme).to_string()),
        ("clinic.color_primary", p.color_primary.clone()),
        ("clinic.color_secondary", p.color_secondary.clone()),
        ("clinic.color_accent", p.color_accent.clone()),
        ("clinic.working_hours", serde_json::to_string(&p.working_hours)?),
        ("clinic.trial_started_at", if p.trial_acknowledged { now.clone() } else { String::new() }),
    ] {
        if v.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT INTO setting(key, value, updated_at, updated_by) VALUES (?1, ?2, ?3, NULL)",
            params![k, v, now],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_and_time_validation() {
        assert!(validate_color("color_primary", "#0e7490").is_ok());
        assert!(validate_color("color_primary", "0e7490").is_err());
        let e = validate_color("color_accent", "#zzzzzz").unwrap_err();
        assert_eq!(e.field(), Some("color_accent"));
        assert!(validate_hhmm("08:30").is_ok());
        assert!(validate_hhmm("24:00").is_err());
        assert!(validate_hhmm("8:30").is_ok());
        assert!(validate_hhmm("nope").is_err());
    }
}
