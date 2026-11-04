// Phase 10 - license state and read-only rules (roadmap 10.8-10.9, ADR-25).
// Plain `//` comments on purpose: the generated TypeScript contract stays free of doc blocks.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseState {
    // No license entered yet (read-only in a production build, open in development and tests).
    Unactivated,
    Trial,
    Active,
    // Past the end date but inside the grace period: works, with a warning bar.
    ExpiredWarning,
    // Valid, but the last online check is old: warn, still works.
    Grace,
    // Read-only from here on.
    Expired,
    // The license was suspended or cancelled by the vendor.
    Revoked,
    // The vendor has not approved this computer yet.
    Pending,
    // The vendor blocked this computer.
    Blocked,
    // The computer's clock went back.
    ClockTampered,
    // Damaged file, another computer's token, or the hardware changed.
    Invalid,
    // Online check overdue beyond the grace period.
    NeedsVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LicenseStatus {
    pub state: LicenseState,
    // True when changes are refused (viewing, printing, export, backup and restore still work).
    pub read_only: bool,
    // "trial" | "paid"
    pub kind: Option<String>,
    // "basic" | "standard" | "pro"
    pub plan: Option<String>,
    // Computers the plan allows (the server counts as one).
    pub computers: u32,
    pub installation_id: String,
    // First 8 characters of this computer's hardware fingerprint, "AB12-CD34": what the vendor compares when approving.
    pub fingerprint_hint: String,
    pub expires_at: Option<String>,
    // Negative after the end date.
    pub days_left: Option<i64>,
    pub verified_at: Option<String>,
    // The next online check is due.
    pub verify_due: bool,
    // Days left until the app becomes read-only for lack of an online check or after the end date.
    pub grace_days_left: Option<i64>,
    // This build knows where the license server is.
    pub server_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ActivateParams {
    // The license key, like 0A1B-2C3D-4E5F-6A7B-8C9D-0E1F.
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct OfflineRequestInfo {
    // Shown to the clinic and sent to the vendor: installation id, fingerprint and computer name.
    pub request_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct OfflineApplyParams {
    // A signed token the vendor made for this computer.
    pub response_code: String,
}
