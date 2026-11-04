//! Phase 9 — dashboard, reports, notifications and global search (roadmap 9.1–9.7). Money is AFN × 100
//! (ADR-06); dates are clinic-local `YYYY-MM-DD`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{CalendarSystem, Language, Translations};

// ───────────────────────────── dashboard (9.1) ─────────────────────────────

/// Whose numbers the dashboard shows.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DashboardScope {
    /// The whole clinic (Owner, administrator, accountant, reception).
    Clinic,
    /// Only the signed-in doctor's own patients, visits and work.
    Doctor,
}

/// What `revenue_today` counts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RevenueBasis {
    /// Money received today, refunds taken off.
    Received,
    /// Work the doctor invoiced today (a doctor does not see the cash desk).
    Invoiced,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DashboardVisit {
    pub appointment_id: String,
    pub patient_id: String,
    pub patient_name: String,
    pub doctor_name: String,
    pub date: String,
    /// Clinic-local `HH:MM`.
    pub time: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BackupStatus {
    pub last_at: Option<String>,
    pub last_verified: bool,
    /// Hours since the last backup, `None` when there is none.
    pub age_hours: Option<i64>,
    /// A backup failed after the last good one.
    pub failed_since: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Dashboard {
    pub date: String,
    pub scope: DashboardScope,
    pub doctor_name: Option<String>,
    /// Different patients booked today (cancelled visits left out).
    pub patients_today: i64,
    pub appointments_today: i64,
    pub appointments_done: i64,
    pub treatments_completed: i64,
    pub revenue_basis: RevenueBasis,
    /// `None`: this user may not see money.
    pub revenue_today: Option<i64>,
    pub outstanding: Option<i64>,
    pub upcoming: Vec<DashboardVisit>,
    /// `None`: this user may not see stock (or it is not theirs to see).
    pub low_stock: Option<i64>,
    pub pending_lab: Option<i64>,
    pub backup: Option<BackupStatus>,
    /// `None` until licensing exists (Phase 10).
    pub license: Option<String>,
}

// ───────────────────────────── reports (9.2–9.5) ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    // Financial (9.2)
    DailyRevenue,
    MonthlyRevenue,
    Payments,
    Outstanding,
    Discounts,
    Refunds,
    DoctorRevenue,
    DoctorCommission,
    ServiceRevenue,
    Expenses,
    ProfitLoss,
    CashClosings,
    // Clinical (9.3)
    Patients,
    Treatments,
    Procedures,
    ToothConditions,
    DoctorActivity,
    FollowUps,
    RecallList,
    ReferralSources,
    // Inventory (9.4)
    Stock,
    LowStock,
    Purchases,
    Consumption,
    Expiry,
    SupplierBalance,
}

impl ReportKind {
    /// "financial" | "clinical" | "inventory": the permission group of the report.
    pub fn group(self) -> &'static str {
        use ReportKind::*;
        match self {
            DailyRevenue | MonthlyRevenue | Payments | Outstanding | Discounts | Refunds | DoctorRevenue
            | DoctorCommission | ServiceRevenue | Expenses | ProfitLoss | CashClosings => "financial",
            Patients | Treatments | Procedures | ToothConditions | DoctorActivity | FollowUps
            | RecallList | ReferralSources => "clinical",
            Stock | LowStock | Purchases | Consumption | Expiry | SupplierBalance => "inventory",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReportParams {
    pub kind: ReportKind,
    /// First day of the period (clinic-local ISO date).
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub doctor_id: Option<String>,
    #[serde(default)]
    pub service_id: Option<String>,
    /// How "monthly" rows are grouped (default Shamsi).
    #[serde(default)]
    pub calendar: Option<CalendarSystem>,
}

/// One value of a report row, typed so the screen formats it (digits, calendar, language) and Excel keeps numbers.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReportCell {
    Empty,
    Text {
        value: String,
    },
    /// An id, a number or a phone: always left-to-right.
    Code {
        value: String,
    },
    Int {
        value: i64,
    },
    /// AFN × 100.
    Money {
        value: i64,
    },
    /// Clinic-local ISO date.
    Date {
        value: String,
    },
    /// A `YYYY-MM` month (Shamsi or Gregorian, as the report says).
    Month {
        value: String,
    },
    Name {
        value: Translations,
    },
    /// A short code the screen translates through `reports.val.<value>` (a status, a kind).
    Tag {
        value: String,
    },
    /// Hundredths of a stock unit (250 = 2.5).
    Qty {
        value: i64,
    },
    /// Parts per ten thousand (2 500 = 25 %).
    Percent {
        value: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReportColumn {
    /// Header text key: `reports.col.<key>`.
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReportResult {
    pub kind: ReportKind,
    pub from: String,
    pub to: String,
    pub columns: Vec<ReportColumn>,
    pub rows: Vec<Vec<ReportCell>>,
    /// Same length as `columns` (`Empty` where a column has no total), or empty.
    pub totals: Vec<ReportCell>,
    /// More rows matched than the report shows.
    pub truncated: bool,
    pub millis: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ExportReportParams {
    pub report: ReportParams,
    pub language: Language,
    /// Sheet and file title, already in the user's language.
    pub title: String,
    /// One header per column, already in the user's language.
    pub headers: Vec<String>,
    /// The words for `Tag` cells (`value` → text in the user's language).
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
}

// ───────────────────────────── notifications (9.6) ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    AppointmentReminder,
    FollowUpDue,
    InstallmentDue,
    LowStock,
    ExpiringItem,
    ExpiredItem,
    LabDelayed,
    BackupFailed,
    BackupOverdue,
    // Phase 10: the trusted USB device is not connected, or looks like a copy; the license needs attention.
    UsbMissing,
    UsbCloned,
    LicenseExpiring,
    LicenseReadOnly,
    LicenseVerify,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum NotificationLevel {
    Info,
    Warning,
    Danger,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NotificationItem {
    /// Stable: the same cause has the same key, so "read" survives a refresh.
    pub key: String,
    pub kind: NotificationKind,
    pub level: NotificationLevel,
    /// Who or what it is about (a patient, an item): the same text in all three languages for a person.
    pub subject: Translations,
    /// Completes the message: a time, a date, days late, or an amount (AFN × 100) for an installment.
    pub detail: Option<String>,
    pub date: Option<String>,
    /// The page that deals with it: "appointments" | "recalls" | "billing" | "inventory" | "backup".
    pub page: String,
    pub patient_id: Option<String>,
    pub read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NotificationList {
    pub items: Vec<NotificationItem>,
    pub unread: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct NotificationKeysParams {
    /// Empty = every current notification.
    #[serde(default)]
    pub keys: Vec<String>,
}

// ───────────────────────────── global search (9.7) ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    Patient,
    Appointment,
    Invoice,
    Receipt,
    LabCase,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchHit {
    pub kind: SearchKind,
    pub id: String,
    pub patient_id: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    pub date: Option<String>,
    /// The page that shows it.
    pub page: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchParams {
    pub query: String,
    /// Per kind (default 5, at most 20).
    #[serde(default)]
    pub per_kind: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub millis: u32,
}
