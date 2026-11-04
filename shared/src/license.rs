// Phase 10 - license, trial and read-only state (roadmap 10.8-10.9).
// Plain `//` comments on purpose: the generated TypeScript contract stays free of doc blocks.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseState {
    // Installed, not activated yet: works normally for a short first period.
    Unactivated,
    Trial,
    Active,
    // The license is valid but the last online check is old: warn, still works.
    Grace,
    // Read-only from here on.
    Expired,
    Revoked,
    // The computer's clock went back.
    ClockTampered,
    // The first period ran out, or the license file is damaged / for another computer.
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
    // "trial" | "basic" | "standard" | "pro"
    pub plan: Option<String>,
    // Computers the plan allows (the server counts as one).
    pub computers: u32,
    pub installation_id: String,
    pub expires_at: Option<String>,
    pub days_left: Option<i64>,
    pub verified_at: Option<String>,
    // The next online check is due (every 7 days).
    pub verify_due: bool,
    // Days left of the offline grace period; None when the last check is recent.
    pub grace_days_left: Option<i64>,
    // Days left of the free first period when not activated.
    pub first_period_days_left: Option<i64>,
    // This build knows where the license server is.
    pub server_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ActivateParams {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct OfflineRequestInfo {
    // Shown to the clinic and sent to the vendor.
    pub request_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct OfflineApplyParams {
    pub response_code: String,
}
