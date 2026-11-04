// Phase 10 - Backup Manager, trusted USB devices, cloud folder and restore (roadmap 10.2-10.7).
// Plain `//` comments on purpose: the generated TypeScript contract stays free of doc blocks.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::BackupInfo;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BackupSettings {
    // Clinic-local time of the daily backup (12-hour in the UI, 24-hour here).
    pub daily_hour: u32,
    pub daily_minute: u32,
    // While a trusted USB device is missing, warn again every this many minutes (10.4).
    pub usb_warn_minutes: u32,
    // Retention (10.5): newest copy of each of the last N days / weeks / months is kept.
    pub keep_daily: u32,
    pub keep_weekly: u32,
    pub keep_monthly: u32,
    // The clinic's own Google Drive / OneDrive sync folder (10.6); None = no cloud copy.
    pub cloud_folder: Option<String>,
    // The database always goes to the cloud folder; images are optional (saves space and internet).
    pub cloud_include_images: bool,
    pub cloud_keep_daily: u32,
    pub cloud_keep_weekly: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CopyStatus {
    Done,
    Pending,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BackupEntry {
    pub info: BackupInfo,
    pub usb: Option<CopyStatus>,
    pub cloud: Option<CopyStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetState {
    // Nothing set up (no trusted device / no cloud folder).
    NotConfigured,
    // The latest backup is there.
    Ok,
    // Waiting: the device is not connected, or the cloud folder is missing.
    Pending,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CloudStatus {
    pub folder: Option<String>,
    pub folder_exists: bool,
    pub writable: bool,
    // "google_drive" | "onedrive" | "other", guessed from the folder name.
    pub provider: String,
    // Whether the sync app seems to be installed; None when it cannot be told.
    pub sync_app_found: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BackupOverview {
    pub last: Option<BackupInfo>,
    // Clinic-local next scheduled run, ISO UTC.
    pub next_at: String,
    pub local_count: u32,
    pub local_bytes: i64,
    pub usb: TargetState,
    pub usb_connected: bool,
    pub usb_pending: u32,
    pub cloud: TargetState,
    pub cloud_pending: u32,
    pub cloud_status: CloudStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsbVerdict {
    // "New Backup Device Detected": nobody trusted it yet.
    New,
    Trusted,
    // The marker was copied to a different stick: no backup is written there.
    Cloned,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UsbVolumeInfo {
    pub mount: String,
    pub vendor: String,
    pub product: String,
    pub label: String,
    pub filesystem: String,
    pub verdict: UsbVerdict,
    pub device_id: Option<String>,
    pub free_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TrustedDeviceInfo {
    pub id: String,
    pub vendor: String,
    pub product: String,
    pub label: Option<String>,
    // Last 4 characters of the hardware serial, enough to tell two sticks apart.
    pub serial_hint: Option<String>,
    pub trusted_at: String,
    pub last_seen_at: Option<String>,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UsbOverview {
    pub volumes: Vec<UsbVolumeInfo>,
    pub trusted: Vec<TrustedDeviceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UsbMountParams {
    pub mount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DeviceIdParams {
    pub device_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackupSource {
    Local,
    Usb,
    Cloud,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RestoreListParams {
    pub source: BackupSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RestoreCandidate {
    pub source: BackupSource,
    pub file_name: String,
    // Folder that holds it (shown on screen; on a new computer it is also what the restore reads).
    pub folder: String,
    pub created_at: Option<String>,
    pub size_bytes: i64,
    // The recovery-wrapped key (.rkey) is beside it.
    pub has_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RestoreTarget {
    pub source: BackupSource,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RestorePreview {
    pub valid: bool,
    // Stable code when not valid: "unreadable" | "wrong_key" | "damaged" | "newer_than_app" | "not_a_backup".
    pub problem: Option<String>,
    pub created_at: Option<String>,
    pub clinic_name: Option<String>,
    pub schema_version: i64,
    pub patients: i64,
    pub appointments: i64,
    pub invoices: i64,
    pub users: i64,
    pub last_activity: Option<String>,
    // The same figures for the data on this computer now, to compare before replacing it.
    pub current_patients: i64,
    pub current_last_activity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RestoreParams {
    pub source: BackupSource,
    pub file_name: String,
    // The Owner's password, asked again before the data is replaced.
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RestoreResult {
    // The safety copy of the data that was replaced.
    pub safety_backup: String,
    pub restored: String,
    pub patients: i64,
}

// New computer (10.7): there is no clinic yet, so the backup is found by folder and opened with the Recovery Key.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ScanBackupsParams {
    // A folder to look in (for example the cloud-synced one); connected USB drives are always scanned.
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RestoreNewParams {
    pub folder: String,
    pub file_name: String,
    pub recovery_key: String,
}
