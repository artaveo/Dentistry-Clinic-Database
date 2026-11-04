//! Backup Manager (roadmap 10.2-10.6).
//!
//! ```text
//! create (VACUUM INTO, separate connection) -> verify (full integrity check on the copy)
//!   -> encrypted already (same SQLCipher key; the Recovery-wrapped key `.rkey` travels with it)
//!   -> local folder -> trusted USB device -> cloud-synced folder
//! ```
//!
//! Attachments are copied *incrementally*: they are stored under their content hash (ADR-12), so a
//! file that already exists at the destination is never copied again. Retention keeps the newest
//! copy of each of the last N days, weeks and months (10.5). A destination that is not reachable
//! (USB not plugged in, cloud folder missing) leaves a `pending` mark; the newest pending backup is
//! transferred by itself as soon as the destination is back (10.4).

use std::collections::HashSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use artaveo_shared::{
    BackupEntry, BackupInfo, BackupOverview, BackupSettings, BackupSource, CloudStatus, CopyStatus,
    ErrorCode, RestoreCandidate, Settings, TargetState, TrustedDeviceInfo, UsbOverview, UsbVerdict,
    UsbVolumeInfo, ValidationRule,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};

use crate::audit::{self, Actor};
use crate::clock::{iso, now, parse, CLINIC_OFFSET};
use crate::config::Config;
use crate::db::{self, KeyFiles};
use crate::error::{CoreError, Result};
use crate::ids::new_id;
use crate::keys::DataKey;
use crate::usb::{self, Marker, Trusted, UsbProvider, UsbVolume, Verdict};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Manual,
    Daily,
    /// The safety copy made just before a restore replaces the data (10.7).
    PreRestore,
}

impl Kind {
    /// Part of the file name.
    pub fn code(self) -> &'static str {
        match self {
            Kind::Manual => "manual",
            Kind::Daily => "daily",
            Kind::PreRestore => "prerestore",
        }
    }

    /// Value of `backup_log.kind` (a CHECK constraint limits it to three words).
    fn log_kind(self) -> &'static str {
        match self {
            Kind::Daily => "daily",
            Kind::Manual | Kind::PreRestore => "manual",
        }
    }
}

const SELECT: &str =
    "SELECT id, file_name, created_at, size_bytes, kind, verified FROM backup_log WHERE deleted_at IS NULL";

fn map(r: &rusqlite::Row<'_>) -> rusqlite::Result<BackupInfo> {
    Ok(BackupInfo {
        id: r.get(0)?,
        file_name: r.get(1)?,
        created_at: r.get(2)?,
        size_bytes: r.get(3)?,
        kind: r.get(4)?,
        verified: r.get::<_, i64>(5)? == 1,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<BackupInfo>> {
    let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY created_at DESC"))?;
    let v = stmt.query_map([], map)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn latest(conn: &Connection) -> Result<Option<BackupInfo>> {
    Ok(conn.query_row(&format!("{SELECT} ORDER BY created_at DESC LIMIT 1"), [], map).optional()?)
}

fn status_from(text: &str) -> CopyStatus {
    match text {
        "done" => CopyStatus::Done,
        "failed" => CopyStatus::Failed,
        "skipped" => CopyStatus::Skipped,
        _ => CopyStatus::Pending,
    }
}

fn status_text(s: CopyStatus) -> &'static str {
    match s {
        CopyStatus::Done => "done",
        CopyStatus::Pending => "pending",
        CopyStatus::Failed => "failed",
        CopyStatus::Skipped => "skipped",
    }
}

/// Every backup with what happened to its USB and cloud copies.
pub fn history(conn: &Connection) -> Result<Vec<BackupEntry>> {
    let mut out = Vec::new();
    for info in list(conn)? {
        let copy = |target: &str| -> Result<Option<CopyStatus>> {
            Ok(conn
                .query_row(
                    "SELECT status FROM backup_copy WHERE backup_id = ?1 AND target = ?2",
                    params![info.id, target],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .map(|s| status_from(&s)))
        };
        let usb = copy("usb")?;
        let cloud = copy("cloud")?;
        out.push(BackupEntry { info, usb, cloud });
    }
    Ok(out)
}

// ───────────────────────────── settings (10.2, 10.4, 10.5, 10.6) ─────────────────────────────

fn read(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

fn num(conn: &Connection, key: &str, default: u32) -> Result<u32> {
    Ok(read(conn, key)?.and_then(|v| v.parse().ok()).unwrap_or(default))
}

fn write(conn: &Connection, key: &str, value: &str, by: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT INTO setting(key, value, updated_at, updated_by) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at,
                                        updated_by = excluded.updated_by",
        params![key, value, iso(now()), by],
    )?;
    Ok(())
}

pub fn get_settings(conn: &Connection) -> Result<BackupSettings> {
    let s: Settings = crate::settings::get(conn)?;
    Ok(BackupSettings {
        daily_hour: s.daily_backup_hour,
        daily_minute: num(conn, "backup_minute", 0)?,
        usb_warn_minutes: num(conn, "backup_usb_warn_minutes", 30)?,
        keep_daily: s.backup_keep_daily,
        keep_weekly: num(conn, "backup_keep_weekly", 4)?,
        keep_monthly: num(conn, "backup_keep_monthly", 3)?,
        cloud_folder: read(conn, "backup_cloud_folder")?.filter(|f| !f.trim().is_empty()),
        cloud_include_images: read(conn, "backup_cloud_images")?.map(|v| v == "1").unwrap_or(true),
        cloud_keep_daily: num(conn, "backup_cloud_keep_daily", 7)?,
        cloud_keep_weekly: num(conn, "backup_cloud_keep_weekly", 4)?,
    })
}

pub fn save_settings(
    conn: &Connection,
    actor: &Actor,
    data_dir: &Path,
    new: &BackupSettings,
) -> Result<BackupSettings> {
    let bad = |field: &str, rule: ValidationRule, text: &str| Err(CoreError::invalid(field, rule, text));
    if new.daily_hour > 23 {
        return bad("daily_hour", ValidationRule::BackupHourRange, "backup hour must be 0-23");
    }
    if new.daily_minute > 59 {
        return bad("daily_minute", ValidationRule::BackupMinuteRange, "backup minute must be 0-59");
    }
    if !(5..=720).contains(&new.usb_warn_minutes) {
        return bad(
            "usb_warn_minutes",
            ValidationRule::UsbIntervalRange,
            "reminder interval must be 5-720 minutes",
        );
    }
    if !(1..=365).contains(&new.keep_daily) {
        return bad("keep_daily", ValidationRule::BackupKeepRange, "daily copies must be 1-365");
    }
    if new.keep_weekly > 104 || new.keep_monthly > 60 {
        let field = if new.keep_weekly > 104 { "keep_weekly" } else { "keep_monthly" };
        return bad(field, ValidationRule::BackupRetentionRange, "weekly copies 0-104, monthly copies 0-60");
    }
    if !(1..=365).contains(&new.cloud_keep_daily) || new.cloud_keep_weekly > 104 {
        let field = if new.cloud_keep_weekly > 104 { "cloud_keep_weekly" } else { "cloud_keep_daily" };
        return bad(field, ValidationRule::BackupRetentionRange, "cloud copies out of range");
    }
    let folder = new.cloud_folder.as_deref().map(str::trim).filter(|f| !f.is_empty());
    if let Some(f) = folder {
        let p = Path::new(f);
        // Writing the cloud copy into our own data folder would copy the backups into themselves.
        if !p.is_absolute() || !p.is_dir() || p.starts_with(data_dir) {
            return bad(
                "cloud_folder",
                ValidationRule::CloudFolder,
                "choose an existing folder outside the app data",
            );
        }
    }
    let before = get_settings(conn)?;
    let by = actor.user_id.as_deref();
    for (k, v) in [
        ("daily_backup_hour", new.daily_hour.to_string()),
        ("backup_minute", new.daily_minute.to_string()),
        ("backup_usb_warn_minutes", new.usb_warn_minutes.to_string()),
        ("backup_keep_daily", new.keep_daily.to_string()),
        ("backup_keep_weekly", new.keep_weekly.to_string()),
        ("backup_keep_monthly", new.keep_monthly.to_string()),
        ("backup_cloud_folder", folder.unwrap_or_default().to_string()),
        ("backup_cloud_images", (new.cloud_include_images as u32).to_string()),
        ("backup_cloud_keep_daily", new.cloud_keep_daily.to_string()),
        ("backup_cloud_keep_weekly", new.cloud_keep_weekly.to_string()),
    ] {
        write(conn, k, &v, by)?;
    }
    let after = get_settings(conn)?;
    audit::record(
        conn,
        actor,
        "backup.settings",
        Some("setting"),
        None,
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

// ───────────────────────────── create (10.2) ─────────────────────────────

/// Creates, verifies and registers a backup. `conn` is the app connection
/// (used only to register/audit); the copy is made from a fresh connection.
pub fn create(
    conn: &Connection,
    key: &DataKey,
    config: &Config,
    kind: Kind,
    actor: &Actor,
) -> Result<BackupInfo> {
    let dir = config.backup_dir();
    fs::create_dir_all(&dir)?;
    let at = now();
    let local = at.to_offset(CLINIC_OFFSET);
    let file_name = format!(
        "artaveo-{:04}{:02}{:02}-{:02}{:02}{:02}-{}.db",
        local.year(),
        local.month() as u8,
        local.day(),
        local.hour(),
        local.minute(),
        local.second(),
        kind.code()
    );
    let path = dir.join(&file_name);

    let result = (|| -> Result<i64> {
        let source = db::connect(&config.db_path(), key)?;
        db::vacuum_into(&source, &path)?;
        drop(source);
        let copy = db::connect(&path, key)?;
        db::full_integrity_check(&copy)?;
        drop(copy);
        fs::copy(KeyFiles::beside(&config.db_path()).recovery, path.with_extension("rkey"))?;
        Ok(fs::metadata(&path)?.len() as i64)
    })();

    let size = match result {
        Ok(size) => size,
        Err(e) => {
            let _ = fs::remove_file(&path);
            let _ = fs::remove_file(path.with_extension("rkey"));
            audit::record(
                conn,
                actor,
                "backup.failed",
                Some("backup"),
                None,
                None,
                Some(&json!({"kind": kind.code(), "error": e.to_string()})),
            )?;
            tracing::error!(error = %e, "backup failed");
            return Err(CoreError::api(ErrorCode::Internal, format!("backup failed: {e}")));
        }
    };

    let info = BackupInfo {
        id: new_id(),
        file_name,
        created_at: iso(at),
        size_bytes: size,
        kind: kind.log_kind().into(),
        verified: true,
    };
    conn.execute(
        "INSERT INTO backup_log(id, file_name, created_at, created_by, size_bytes, kind, verified)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
        params![info.id, info.file_name, info.created_at, actor.user_id, info.size_bytes, info.kind],
    )?;
    audit::record(conn, actor, "backup.create", Some("backup"), Some(&info.id), None, Some(&json!(info)))?;
    tracing::info!(file = %info.file_name, size = info.size_bytes, "backup created");
    Ok(info)
}

/// Files in the backup folder that the register does not know (after a restore the register came back
/// from an older state): they are listed again so retention and the history still see them.
pub fn reconcile(conn: &Connection, dir: &Path) -> Result<u32> {
    let mut added = 0;
    for (name, at) in list_folder(dir) {
        let known: i64 =
            conn.query_row("SELECT COUNT(*) FROM backup_log WHERE file_name = ?1", [&name], |r| r.get(0))?;
        if known > 0 {
            continue;
        }
        let size = fs::metadata(dir.join(&name)).map(|m| m.len() as i64).unwrap_or(0);
        let kind = if name.ends_with("-daily.db") { "daily" } else { "manual" };
        conn.execute(
            "INSERT INTO backup_log(id, file_name, created_at, created_by, size_bytes, kind, verified)
             VALUES (?1, ?2, ?3, NULL, ?4, ?5, 1)",
            params![new_id(), name, iso(at), size, kind],
        )?;
        added += 1;
    }
    Ok(added)
}

// ───────────────────────────── retention (10.5) ─────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct Keep {
    pub daily: u32,
    pub weekly: u32,
    pub monthly: u32,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
    pub at: OffsetDateTime,
    /// A safety copy made before a restore: the newest few are never removed.
    pub protected: bool,
}

/// `artaveo-20261104-190000-daily.db` -> the clinic-local instant in the name.
pub fn parse_name(name: &str) -> Option<OffsetDateTime> {
    let rest = name.strip_prefix("artaveo-")?;
    if rest.as_bytes().get(8) != Some(&b'-') {
        return None;
    }
    let (date, time_part) = (rest.get(0..8)?, rest.get(9..15)?);
    let n = |s: &str| s.parse::<u32>().ok();
    let d = time::Date::from_calendar_date(
        date.get(0..4)?.parse().ok()?,
        time::Month::try_from(n(date.get(4..6)?)? as u8).ok()?,
        n(date.get(6..8)?)? as u8,
    )
    .ok()?;
    let t = time::Time::from_hms(
        n(time_part.get(0..2)?)? as u8,
        n(time_part.get(2..4)?)? as u8,
        n(time_part.get(4..6)?)? as u8,
    )
    .ok()?;
    Some(time::PrimitiveDateTime::new(d, t).assume_offset(CLINIC_OFFSET))
}

/// The backups in a folder (`artaveo-*.db`), with the time in their names.
pub fn list_folder(dir: &Path) -> Vec<(String, OffsetDateTime)> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<(String, OffsetDateTime)> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".db"))
        .filter_map(|n| parse_name(&n).map(|at| (n, at)))
        .collect();
    out.sort_by_key(|x| std::cmp::Reverse(x.1));
    out
}

/// However the retention is set, this many of the newest backups always stay (a manual backup made
/// before a risky change must not vanish because another one was made later the same day).
pub const NEWEST_ALWAYS_KEPT: usize = 3;

/// Names to keep: the newest copy of each of the last `daily` days, `weekly` weeks (Monday start) and
/// `monthly` months, always the newest [`NEWEST_ALWAYS_KEPT`], and the newest three safety copies.
pub fn select_keep(items: &[Item], at: OffsetDateTime, keep: Keep) -> HashSet<String> {
    let today = at.to_offset(CLINIC_OFFSET).date();
    let today_week = today - Duration::days(today.weekday().number_days_from_monday() as i64);
    let mut sorted: Vec<&Item> = items.iter().collect();
    sorted.sort_by_key(|i| std::cmp::Reverse(i.at));
    let mut kept = HashSet::new();
    for it in sorted.iter().take(NEWEST_ALWAYS_KEPT) {
        kept.insert(it.name.clone());
    }
    for it in sorted.iter().filter(|i| i.protected).take(3) {
        kept.insert(it.name.clone());
    }
    let (mut days, mut weeks, mut months) = (HashSet::new(), HashSet::new(), HashSet::new());
    for it in &sorted {
        let d = it.at.to_offset(CLINIC_OFFSET).date();
        let age_days = (today - d).whole_days();
        if age_days < 0 {
            kept.insert(it.name.clone()); // the clock was wrong when it was made: do not guess
            continue;
        }
        if age_days < keep.daily as i64 && days.insert(d) {
            kept.insert(it.name.clone());
        }
        let week_start = d - Duration::days(d.weekday().number_days_from_monday() as i64);
        let weeks_ago = (today_week - week_start).whole_days() / 7;
        if weeks_ago < keep.weekly as i64 && weeks.insert(week_start) {
            kept.insert(it.name.clone());
        }
        let months_ago =
            (today.year() as i64 - d.year() as i64) * 12 + (today.month() as i64 - d.month() as i64);
        if months_ago < keep.monthly as i64 && months.insert((d.year(), d.month() as u8)) {
            kept.insert(it.name.clone());
        }
    }
    kept
}

/// Removes the backups in `dir` that retention does not keep; returns their names.
pub fn retain_folder(dir: &Path, at: OffsetDateTime, keep: Keep) -> Result<Vec<String>> {
    let items: Vec<Item> = list_folder(dir)
        .into_iter()
        .map(|(name, at)| Item { protected: name.contains("-prerestore."), name, at })
        .collect();
    let kept = select_keep(&items, at, keep);
    let mut removed = Vec::new();
    for it in items.iter().filter(|i| !kept.contains(&i.name)) {
        let p = dir.join(&it.name);
        fs::remove_file(&p)?;
        let _ = fs::remove_file(p.with_extension("rkey"));
        removed.push(it.name.clone());
    }
    Ok(removed)
}

/// Retention for the local folder; the register marks what was removed.
pub fn apply_retention(conn: &Connection, config: &Config) -> Result<u32> {
    let s = get_settings(conn)?;
    let keep = Keep { daily: s.keep_daily, weekly: s.keep_weekly, monthly: s.keep_monthly };
    let removed = retain_folder(&config.backup_dir(), now(), keep)?;
    for name in &removed {
        conn.execute(
            "UPDATE backup_log SET deleted_at = ?1 WHERE file_name = ?2 AND deleted_at IS NULL",
            params![iso(now()), name],
        )?;
    }
    Ok(removed.len() as u32)
}

// ───────────────────────────── schedule ─────────────────────────────

/// Daily rule: due when there is no backup in the last 24 h, or when the configured local time has
/// passed and today has no backup yet.
pub fn is_due(latest: Option<&BackupInfo>, hour: u32, minute: u32, at: OffsetDateTime) -> bool {
    let Some(last) = latest.and_then(|b| parse(&b.created_at)) else { return true };
    if at - last >= Duration::hours(24) {
        return true;
    }
    let local_now = at.to_offset(CLINIC_OFFSET);
    let local_last = last.to_offset(CLINIC_OFFSET);
    let passed = (local_now.hour() as u32, local_now.minute() as u32) >= (hour, minute);
    passed && local_last.date() < local_now.date()
}

/// When the next automatic backup runs (now, if one is already due).
pub fn next_run(latest: Option<&BackupInfo>, hour: u32, minute: u32, at: OffsetDateTime) -> OffsetDateTime {
    if is_due(latest, hour, minute, at) {
        return at;
    }
    let local = at.to_offset(CLINIC_OFFSET);
    let when = time::Time::from_hms(hour as u8, minute as u8, 0).unwrap_or(time::Time::MIDNIGHT);
    let today = local.replace_time(when);
    if today > local {
        today
    } else {
        today + Duration::days(1)
    }
}

// ───────────────────────────── destinations (10.3, 10.4, 10.6) ─────────────────────────────

fn file_sha256(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut File::open(path)?, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

/// Copies through a `.part` file and checks the result byte for byte (hash), so a pulled USB stick
/// or a full disk never leaves a backup that only looks complete.
fn copy_checked(src: &Path, dst: &Path) -> Result<()> {
    if let Some(dir) = dst.parent() {
        fs::create_dir_all(dir)?;
    }
    let part = dst.with_extension("part");
    let result = (|| -> Result<()> {
        fs::copy(src, &part)?;
        if file_sha256(src)? != file_sha256(&part)? {
            return Err(CoreError::api(ErrorCode::Internal, "the copy does not match the original"));
        }
        fs::rename(&part, dst)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

/// Copies attachment files that the destination does not have yet. Returns how many were copied.
pub fn sync_attachments(src: &Path, dst: &Path) -> Result<u32> {
    if !src.is_dir() {
        return Ok(0);
    }
    let mut copied = 0;
    for sub in fs::read_dir(src)? {
        let sub = sub?;
        if !sub.file_type()?.is_dir() {
            continue;
        }
        for file in fs::read_dir(sub.path())? {
            let file = file?;
            let name = file.file_name();
            if !name.to_string_lossy().ends_with(".bin") {
                continue;
            }
            let to = dst.join(sub.file_name()).join(&name);
            if !to.exists() {
                copy_checked(&file.path(), &to)?;
                copied += 1;
            }
        }
    }
    Ok(copied)
}

/// Where one destination keeps its files.
struct Dest {
    dir: PathBuf,
    /// `Some` when images go along.
    attachments: Option<PathBuf>,
    keep: Keep,
}

fn copy_to(dest: &Dest, config: &Config, info: &BackupInfo) -> Result<()> {
    let src = config.backup_dir().join(&info.file_name);
    if !src.exists() {
        return Err(CoreError::api(ErrorCode::NotFound, "the local backup file is gone"));
    }
    fs::create_dir_all(&dest.dir)?;
    if let Some(free) = usb::free_bytes(&dest.dir) {
        let need = info.size_bytes + info.size_bytes / 10 + 16 * 1024 * 1024;
        if free < need {
            return Err(CoreError::api(ErrorCode::Internal, "not enough free space on the destination"));
        }
    }
    copy_checked(&src, &dest.dir.join(&info.file_name))?;
    if src.with_extension("rkey").exists() {
        copy_checked(&src.with_extension("rkey"), &dest.dir.join(&info.file_name).with_extension("rkey"))?;
    }
    if let Some(att) = &dest.attachments {
        sync_attachments(&config.attachments_dir(), att)?;
    }
    retain_folder(&dest.dir, now(), dest.keep)?;
    Ok(())
}

pub fn clinic_id(conn: &Connection) -> Result<String> {
    Ok(conn
        .query_row("SELECT value FROM db_meta WHERE key = 'clinic_id'", [], |r| r.get(0))
        .optional()?
        .unwrap_or_default())
}

/// A trusted device row.
#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub id: String,
    pub vendor: String,
    pub product: String,
    pub serial: Option<String>,
    pub label: Option<String>,
    pub trusted_at: String,
    pub last_seen_at: Option<String>,
}

pub fn trusted_devices(conn: &Connection) -> Result<Vec<DeviceRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, vendor, product, hardware_serial, label, trusted_at, last_seen_at
         FROM backup_device WHERE deleted_at IS NULL ORDER BY trusted_at",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(DeviceRow {
                id: r.get(0)?,
                vendor: r.get(1)?,
                product: r.get(2)?,
                serial: r.get(3)?,
                label: r.get(4)?,
                trusted_at: r.get(5)?,
                last_seen_at: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// A connected USB volume with what the clinic's records say about it.
#[derive(Debug, Clone)]
pub struct Detected {
    pub volume: UsbVolume,
    pub verdict: Verdict,
    pub marker: Option<Marker>,
}

pub fn detect(conn: &Connection, key: &DataKey, provider: &dyn UsbProvider) -> Result<Vec<Detected>> {
    let clinic = clinic_id(conn)?;
    let trusted: Vec<Trusted> = trusted_devices(conn)?
        .into_iter()
        .map(|d| Trusted { device_id: d.id, hardware_serial: d.serial })
        .collect();
    Ok(provider
        .volumes()
        .into_iter()
        .map(|volume| {
            let marker = Marker::read_verified(&volume.mount, &clinic, key);
            let verdict = usb::verify(&volume, marker.as_ref(), &trusted);
            Detected { volume, verdict, marker }
        })
        .collect())
}

fn set_copy(
    conn: &Connection,
    backup_id: &str,
    target: &str,
    status: CopyStatus,
    device: Option<&str>,
    detail: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO backup_copy(backup_id, target, status, device_id, updated_at, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(backup_id, target) DO UPDATE SET status = excluded.status, device_id = excluded.device_id,
                                                      updated_at = excluded.updated_at, detail = excluded.detail",
        params![backup_id, target, status_text(status), device, iso(now()), detail],
    )?;
    Ok(())
}

/// What happened when one destination was tried: `None` = nothing to do (not configured).
struct Outcome {
    status: CopyStatus,
    device: Option<String>,
    detail: Option<String>,
}

fn deliver_usb(
    conn: &Connection,
    key: &DataKey,
    config: &Config,
    provider: &dyn UsbProvider,
    info: &BackupInfo,
) -> Result<Option<Outcome>> {
    if trusted_devices(conn)?.is_empty() {
        return Ok(None);
    }
    let s = get_settings(conn)?;
    let keep = Keep { daily: s.keep_daily, weekly: s.keep_weekly, monthly: s.keep_monthly };
    let mut done: Option<String> = None;
    let mut failed: Option<String> = None;
    for d in detect(conn, key, provider)? {
        if d.verdict != Verdict::Trusted {
            continue;
        }
        let Some(id) = d.marker.as_ref().map(|m| m.device_id.clone()) else { continue };
        let folder = usb::backup_folder(&d.volume.mount);
        let dest = Dest { attachments: Some(folder.join("attachments")), dir: folder, keep };
        match copy_to(&dest, config, info) {
            Ok(()) => {
                conn.execute(
                    "UPDATE backup_device SET last_seen_at = ?1 WHERE id = ?2",
                    params![iso(now()), id],
                )?;
                done = Some(id);
            }
            Err(e) => {
                tracing::warn!(error = %e, "copy to the USB device failed");
                failed = Some(e.to_string());
            }
        }
    }
    Ok(Some(match (done, failed) {
        (Some(id), _) => Outcome { status: CopyStatus::Done, device: Some(id), detail: None },
        (None, Some(e)) => Outcome { status: CopyStatus::Failed, device: None, detail: Some(e) },
        (None, None) => Outcome { status: CopyStatus::Pending, device: None, detail: None },
    }))
}

/// `<cloud folder>/ArtaveoDental`: the app's own sub-folder inside the folder the Drive/OneDrive app syncs.
pub fn cloud_dir(folder: &str) -> PathBuf {
    Path::new(folder).join("ArtaveoDental")
}

fn deliver_cloud(conn: &Connection, config: &Config, info: &BackupInfo) -> Result<Option<Outcome>> {
    let s = get_settings(conn)?;
    let Some(folder) = s.cloud_folder.clone() else { return Ok(None) };
    if !Path::new(&folder).is_dir() {
        return Ok(Some(Outcome {
            status: CopyStatus::Pending,
            device: None,
            detail: Some("folder missing".into()),
        }));
    }
    let dir = cloud_dir(&folder);
    let dest = Dest {
        attachments: s.cloud_include_images.then(|| dir.join("attachments")),
        dir,
        keep: Keep { daily: s.cloud_keep_daily, weekly: s.cloud_keep_weekly, monthly: 0 },
    };
    Ok(Some(match copy_to(&dest, config, info) {
        Ok(()) => Outcome { status: CopyStatus::Done, device: None, detail: None },
        Err(e) => {
            tracing::warn!(error = %e, "copy to the cloud folder failed");
            Outcome { status: CopyStatus::Failed, device: None, detail: Some(e.to_string()) }
        }
    }))
}

fn record(conn: &Connection, backup_id: &str, target: &str, o: Option<Outcome>) -> Result<()> {
    if let Some(o) = o {
        set_copy(conn, backup_id, target, o.status, o.device.as_deref(), o.detail.as_deref())?;
        if o.status == CopyStatus::Done {
            // The newest backup holds everything the waiting older ones did.
            conn.execute(
                "UPDATE backup_copy SET status = 'skipped', detail = 'superseded', updated_at = ?1
                 WHERE target = ?2 AND backup_id <> ?3 AND status IN ('pending', 'failed')",
                params![iso(now()), target, backup_id],
            )?;
        }
    }
    Ok(())
}

/// After a backup was made locally: USB, then the cloud folder (10.2). A destination that cannot be
/// reached is marked pending, never an error: the local backup is already safe.
pub fn distribute(
    conn: &Connection,
    key: &DataKey,
    config: &Config,
    provider: &dyn UsbProvider,
    info: &BackupInfo,
) -> Result<()> {
    record(conn, &info.id, "usb", deliver_usb(conn, key, config, provider, info)?)?;
    record(conn, &info.id, "cloud", deliver_cloud(conn, config, info)?)?;
    Ok(())
}

/// Brings the newest backup to a destination that was missing when it was made (10.4:
/// `USB Detected -> Pending Backup -> Automatic Transfer`). Older pending ones are then marked
/// skipped (see `record`): the newest backup already contains them. Returns how many transfers worked.
pub fn transfer_pending(
    conn: &Connection,
    key: &DataKey,
    config: &Config,
    provider: &dyn UsbProvider,
) -> Result<u32> {
    let mut done = 0;
    for target in ["usb", "cloud"] {
        let mut stmt = conn.prepare(
            "SELECT c.backup_id FROM backup_copy c JOIN backup_log l ON l.id = c.backup_id
             WHERE c.target = ?1 AND l.deleted_at IS NULL
               AND (c.status = 'pending' OR (c.status = 'failed' AND c.updated_at < ?2))
             ORDER BY l.created_at DESC",
        )?;
        // A failed copy (disk full, ...) is tried again, but not on every tick of the scheduler.
        let retry_before = iso(now() - Duration::minutes(30));
        let ids: Vec<String> =
            stmt.query_map(params![target, retry_before], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        let Some(newest) = ids.first() else { continue };
        let info = conn.query_row(&format!("{SELECT} AND id = ?1"), [newest], map)?;
        let outcome = if target == "usb" {
            deliver_usb(conn, key, config, provider, &info)?
        } else {
            deliver_cloud(conn, config, &info)?
        };
        if outcome.as_ref().is_some_and(|o| o.status == CopyStatus::Done) {
            done += 1;
        }
        record(conn, newest, target, outcome)?;
    }
    Ok(done)
}

// ───────────────────────────── USB management (10.3) ─────────────────────────────

pub fn usb_overview(conn: &Connection, key: &DataKey, provider: &dyn UsbProvider) -> Result<UsbOverview> {
    let detected = detect(conn, key, provider)?;
    let connected: HashSet<String> = detected
        .iter()
        .filter(|d| d.verdict == Verdict::Trusted)
        .filter_map(|d| d.marker.as_ref().map(|m| m.device_id.clone()))
        .collect();
    let volumes = detected
        .iter()
        .map(|d| UsbVolumeInfo {
            mount: d.volume.mount.display().to_string(),
            vendor: d.volume.vendor.clone(),
            product: d.volume.product.clone(),
            label: d.volume.label.clone(),
            filesystem: d.volume.filesystem.clone(),
            verdict: match d.verdict {
                Verdict::Trusted => UsbVerdict::Trusted,
                Verdict::Cloned => UsbVerdict::Cloned,
                Verdict::Unknown => UsbVerdict::New,
            },
            device_id: d.marker.as_ref().map(|m| m.device_id.clone()),
            free_bytes: usb::free_bytes(&d.volume.mount),
        })
        .collect();
    let trusted = trusted_devices(conn)?
        .into_iter()
        .map(|d| TrustedDeviceInfo {
            connected: connected.contains(&d.id),
            serial_hint: d
                .serial
                .as_deref()
                .map(|s| s.chars().skip(s.chars().count().saturating_sub(4)).collect::<String>()),
            id: d.id,
            vendor: d.vendor,
            product: d.product,
            label: d.label,
            trusted_at: d.trusted_at,
            last_seen_at: d.last_seen_at,
        })
        .collect();
    Ok(UsbOverview { volumes, trusted })
}

/// "Use for Backup": writes the signed marker on the device and remembers it (10.3).
pub fn trust_device(
    conn: &Connection,
    key: &DataKey,
    provider: &dyn UsbProvider,
    actor: &Actor,
    mount: &str,
) -> Result<()> {
    let detected = detect(conn, key, provider)?;
    let Some(d) = detected.into_iter().find(|d| d.volume.mount.display().to_string() == mount) else {
        return Err(CoreError::invalid(
            "mount",
            ValidationRule::UsbMountUnknown,
            "that drive is not connected",
        ));
    };
    if d.verdict == Verdict::Cloned {
        return Err(CoreError::invalid(
            "mount",
            ValidationRule::UsbMountUnknown,
            "this drive carries a copied marker; format it first",
        ));
    }
    if d.verdict == Verdict::Trusted {
        return Ok(());
    }
    let marker = Marker::issue(&clinic_id(conn)?, key);
    marker.write(&d.volume.mount)?;
    conn.execute(
        "INSERT INTO backup_device(id, vendor, product, hardware_serial, label, trusted_at, trusted_by, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?6)",
        params![
            marker.device_id,
            d.volume.vendor,
            d.volume.product,
            d.volume.hardware_serial.as_deref().and_then(usb::credible_serial),
            d.volume.label,
            iso(now()),
            actor.user_id
        ],
    )?;
    audit::record(
        conn,
        actor,
        "backup.usb_trust",
        Some("backup_device"),
        Some(&marker.device_id),
        None,
        Some(&json!({"vendor": d.volume.vendor, "product": d.volume.product})),
    )?;
    Ok(())
}

pub fn forget_device(conn: &Connection, actor: &Actor, device_id: &str) -> Result<()> {
    let changed = conn.execute(
        "UPDATE backup_device SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![iso(now()), device_id],
    )?;
    if changed == 0 {
        return Err(CoreError::api(ErrorCode::NotFound, "unknown backup device"));
    }
    audit::record(conn, actor, "backup.usb_forget", Some("backup_device"), Some(device_id), None, None)?;
    Ok(())
}

// ───────────────────────────── cloud folder (10.6) ─────────────────────────────

pub fn cloud_status(folder: Option<&str>) -> CloudStatus {
    let Some(folder) = folder else {
        return CloudStatus {
            folder: None,
            folder_exists: false,
            writable: false,
            provider: "other".into(),
            sync_app_found: None,
        };
    };
    let lower = folder.to_lowercase();
    let provider =
        if lower.contains("google drive") || lower.contains("googledrive") || lower.contains("my drive") {
            "google_drive"
        } else if lower.contains("onedrive") {
            "onedrive"
        } else {
            "other"
        };
    let exists = Path::new(folder).is_dir();
    let writable = exists && {
        let probe = Path::new(folder).join(".artaveo-write-test");
        let ok = fs::write(&probe, b"x").is_ok();
        let _ = fs::remove_file(&probe);
        ok
    };
    CloudStatus {
        folder: Some(folder.to_string()),
        folder_exists: exists,
        writable,
        provider: provider.into(),
        sync_app_found: sync_app_found(provider),
    }
}

/// Whether the Drive / OneDrive desktop app seems to be installed, when this can be told.
fn sync_app_found(provider: &str) -> Option<bool> {
    if !cfg!(windows) {
        return None;
    }
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let candidates: Vec<PathBuf> = match provider {
        "google_drive" => ["ProgramFiles", "ProgramFiles(x86)"]
            .into_iter()
            .filter_map(env)
            .map(|p| p.join("Google").join("Drive File Stream"))
            .chain(env("ProgramFiles").map(|p| p.join("Google").join("Drive")))
            .collect(),
        "onedrive" => env("LOCALAPPDATA").map(|p| p.join("Microsoft").join("OneDrive")).into_iter().collect(),
        _ => return None,
    };
    Some(candidates.iter().any(|p| p.exists()))
}

// ───────────────────────────── overview ─────────────────────────────

fn target_state(
    conn: &Connection,
    target: &str,
    configured: bool,
    newest: Option<&BackupInfo>,
) -> Result<TargetState> {
    if !configured {
        return Ok(TargetState::NotConfigured);
    }
    let Some(b) = newest else { return Ok(TargetState::Pending) };
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM backup_copy WHERE backup_id = ?1 AND target = ?2",
            params![b.id, target],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match status.as_deref() {
        Some("done") => TargetState::Ok,
        Some("failed") => TargetState::Failed,
        _ => TargetState::Pending,
    })
}

fn pending_count(conn: &Connection, target: &str) -> Result<u32> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM backup_copy c JOIN backup_log l ON l.id = c.backup_id
         WHERE c.target = ?1 AND c.status IN ('pending', 'failed') AND l.deleted_at IS NULL",
        [target],
        |r| r.get::<_, u32>(0),
    )?)
}

pub fn overview(conn: &Connection, key: &DataKey, provider: &dyn UsbProvider) -> Result<BackupOverview> {
    let s = get_settings(conn)?;
    let last = latest(conn)?;
    let all = list(conn)?;
    let devices = trusted_devices(conn)?;
    let usb_connected =
        !devices.is_empty() && detect(conn, key, provider)?.iter().any(|d| d.verdict == Verdict::Trusted);
    let cloud_status = cloud_status(s.cloud_folder.as_deref());
    let cloud = target_state(conn, "cloud", s.cloud_folder.is_some(), last.as_ref())?;
    Ok(BackupOverview {
        next_at: iso(next_run(last.as_ref(), s.daily_hour, s.daily_minute, now())),
        local_count: all.len() as u32,
        local_bytes: all.iter().map(|b| b.size_bytes).sum(),
        usb: target_state(conn, "usb", !devices.is_empty(), last.as_ref())?,
        usb_connected,
        usb_pending: pending_count(conn, "usb")?,
        cloud,
        cloud_pending: pending_count(conn, "cloud")?,
        cloud_status,
        last,
    })
}

// ───────────────────────────── restore candidates (10.7) ─────────────────────────────

/// Backups found in `dir`, newest first.
pub fn candidates_in(source: BackupSource, dir: &Path) -> Vec<RestoreCandidate> {
    list_folder(dir)
        .into_iter()
        .map(|(name, at)| {
            let path = dir.join(&name);
            RestoreCandidate {
                source,
                size_bytes: fs::metadata(&path).map(|m| m.len() as i64).unwrap_or(0),
                has_key: path.with_extension("rkey").exists(),
                created_at: Some(iso(at)),
                folder: dir.display().to_string(),
                file_name: name,
            }
        })
        .collect()
}

/// The folders a restore may read from, for one kind of source.
pub fn source_folders(
    conn: &Connection,
    key: &DataKey,
    config: &Config,
    provider: &dyn UsbProvider,
    source: BackupSource,
) -> Result<Vec<PathBuf>> {
    Ok(match source {
        BackupSource::Local => vec![config.backup_dir()],
        BackupSource::Usb => detect(conn, key, provider)?
            .into_iter()
            .filter(|d| d.verdict == Verdict::Trusted)
            .map(|d| usb::backup_folder(&d.volume.mount))
            .collect(),
        BackupSource::Cloud => {
            get_settings(conn)?.cloud_folder.map(|f| vec![cloud_dir(&f)]).unwrap_or_default()
        }
    })
}

/// A backup file name from a request must be a plain `artaveo-*.db` name, never a path.
pub fn safe_file_name(name: &str) -> Result<&str> {
    if parse_name(name).is_none() || name.contains(['/', '\\']) || !name.ends_with(".db") {
        return Err(CoreError::invalid(
            "file_name",
            ValidationRule::BackupFileUnknown,
            "not a backup file name",
        ));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn b(at: &str) -> BackupInfo {
        BackupInfo {
            id: "x".into(),
            file_name: "f".into(),
            created_at: at.into(),
            size_bytes: 1,
            kind: "daily".into(),
            verified: true,
        }
    }

    #[test]
    fn daily_due_rules() {
        // Yesterday 19:05 Kabul (14:35Z); now today 18:00 Kabul -> not due (< 24 h, before 19:00).
        let last = b("2026-10-01T14:35:00.000Z");
        assert!(is_due(None, 19, 0, now()));
        assert!(!is_due(Some(&last), 19, 0, datetime!(2026-10-02 13:30 UTC)));
        // Today 19:10 Kabul -> due.
        assert!(is_due(Some(&last), 19, 0, datetime!(2026-10-02 14:40 UTC)));
        // The minute counts: 19:10 is before a 19:30 schedule, 19:40 is after.
        assert!(!is_due(Some(&last), 19, 30, datetime!(2026-10-02 14:40 UTC)));
        assert!(is_due(Some(&last), 19, 30, datetime!(2026-10-02 15:10 UTC)));
        // Already backed up today at 19:01 Kabul -> not due at 22:00.
        let today = b("2026-10-02T14:31:00.000Z");
        assert!(!is_due(Some(&today), 19, 0, datetime!(2026-10-02 17:30 UTC)));
        // App closed for two days -> due immediately in the morning.
        assert!(is_due(Some(&last), 19, 0, datetime!(2026-10-03 04:00 UTC)));
    }

    #[test]
    fn next_run_is_the_coming_schedule() {
        let today = b("2026-10-02T14:31:00.000Z"); // 19:01 Kabul
                                                   // 22:00 Kabul on the 2nd: next is 19:00 on the 3rd Kabul = 14:30Z.
        assert_eq!(
            next_run(Some(&today), 19, 0, datetime!(2026-10-02 17:30 UTC)),
            datetime!(2026-10-03 14:30 UTC)
        );
        // Not yet done today, before the time: later today.
        let old = b("2026-10-01T14:31:00.000Z");
        assert_eq!(
            next_run(Some(&old), 19, 0, datetime!(2026-10-02 06:00 UTC)),
            datetime!(2026-10-02 14:30 UTC)
        );
        // Due now: now.
        assert_eq!(next_run(None, 19, 0, datetime!(2026-10-02 06:00 UTC)), datetime!(2026-10-02 06:00 UTC));
    }

    #[test]
    fn names_round_trip() {
        let at = parse_name("artaveo-20261104-190500-daily.db").unwrap();
        assert_eq!(at, datetime!(2026-11-04 14:35:00 UTC));
        assert!(parse_name("pre-migration-v1-to-v2-123.db").is_none());
        assert!(parse_name("artaveo-2026110x-190500-daily.db").is_none());
        assert!(parse_name("artaveo-20261304-190500-daily.db").is_none());
        assert!(safe_file_name("artaveo-20261104-190500-daily.db").is_ok());
        assert!(safe_file_name("../artaveo-20261104-190500-daily.db").is_err());
        assert!(safe_file_name("artaveo-20261104-190500-daily.exe").is_err());
    }

    fn item(name: &str, at: OffsetDateTime) -> Item {
        Item { name: name.into(), at, protected: name.contains("prerestore") }
    }

    #[test]
    fn retention_keeps_daily_weekly_and_monthly_copies() {
        let at = datetime!(2026-11-04 08:00 UTC);
        // One copy a day for 120 days, newest first by name.
        let items: Vec<Item> = (0..120).map(|i| item(&format!("d{i:03}"), at - Duration::days(i))).collect();
        let kept = select_keep(&items, at, Keep { daily: 14, weekly: 4, monthly: 3 });
        for i in 0..14 {
            assert!(kept.contains(&format!("d{i:03}")), "the last 14 days stay: d{i}");
        }
        // Copies between day 14 and 120 survive only as weekly (4 weeks) or monthly (3 months) representatives.
        let older = (14..120).filter(|i| kept.contains(&format!("d{i:03}"))).count();
        assert!(older <= 4 + 3, "weekly and monthly representatives only, got {older}");
        assert!(older >= 1);
        assert!(!kept.contains("d119"));
        // Zero everywhere still keeps the newest three copies.
        let only = select_keep(&items, at, Keep { daily: 0, weekly: 0, monthly: 0 });
        assert_eq!(only.len(), 3);
        assert!(only.contains("d000") && only.contains("d001") && only.contains("d002"));
    }

    #[test]
    fn retention_keeps_one_per_day_and_safety_copies() {
        let at = datetime!(2026-11-04 08:00 UTC);
        let items = vec![
            item("a-late", at - Duration::hours(8)),
            item("a-night", at - Duration::hours(7)),
            item("a-morning", at - Duration::hours(6)),
            item("a-newest", at - Duration::hours(1)),
            item("b-yesterday", at - Duration::days(1)),
            item("c-pre-1", at - Duration::days(200)),
            item("c-prerestore-2", at - Duration::days(201)),
            item("c-prerestore-3", at - Duration::days(202)),
        ];
        let kept = select_keep(&items, at, Keep { daily: 3, weekly: 0, monthly: 0 });
        assert!(
            kept.contains("a-newest") && kept.contains("a-morning") && kept.contains("a-night"),
            "newest three"
        );
        assert!(!kept.contains("a-late"), "a fourth copy of the same day goes");
        assert!(kept.contains("b-yesterday"), "yesterday's newest");
        assert!(kept.contains("c-prerestore-2") && kept.contains("c-prerestore-3"), "safety copies stay");
        assert!(!kept.contains("c-pre-1"));
    }

    #[test]
    fn folder_retention_removes_the_files_and_their_keys() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "artaveo-20260101-100000-daily",
            "artaveo-20261102-100000-daily",
            "artaveo-20261103-100000-daily",
            "artaveo-20261104-100000-daily",
        ] {
            fs::write(dir.path().join(format!("{name}.db")), b"x").unwrap();
            fs::write(dir.path().join(format!("{name}.rkey")), b"k").unwrap();
        }
        let removed = retain_folder(
            dir.path(),
            datetime!(2026-11-04 08:00 UTC),
            Keep { daily: 3, weekly: 0, monthly: 0 },
        )
        .unwrap();
        assert_eq!(removed, vec!["artaveo-20260101-100000-daily.db".to_string()]);
        assert!(!dir.path().join("artaveo-20260101-100000-daily.rkey").exists());
        assert!(dir.path().join("artaveo-20261104-100000-daily.rkey").exists());
    }

    #[test]
    fn attachments_are_copied_once() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        fs::create_dir_all(src.path().join("ab")).unwrap();
        fs::write(src.path().join("ab").join("abcd.bin"), b"one").unwrap();
        assert_eq!(sync_attachments(src.path(), dst.path()).unwrap(), 1);
        assert_eq!(sync_attachments(src.path(), dst.path()).unwrap(), 0, "already there");
        fs::write(src.path().join("ab").join("abef.bin"), b"two").unwrap();
        assert_eq!(sync_attachments(src.path(), dst.path()).unwrap(), 1, "only the new file");
        assert_eq!(fs::read(dst.path().join("ab").join("abef.bin")).unwrap(), b"two");
    }

    #[test]
    fn cloud_provider_is_guessed_from_the_folder_name() {
        assert_eq!(cloud_status(Some("/nonexistent/Google Drive/My Drive")).provider, "google_drive");
        assert_eq!(cloud_status(Some("/nonexistent/OneDrive - Clinic")).provider, "onedrive");
        assert_eq!(cloud_status(Some("/nonexistent/Dropbox")).provider, "other");
        assert!(!cloud_status(Some("/nonexistent/x")).folder_exists);
        let dir = tempfile::tempdir().unwrap();
        let s = cloud_status(dir.path().to_str());
        assert!(s.folder_exists && s.writable);
    }
}
