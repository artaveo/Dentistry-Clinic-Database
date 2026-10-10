//! API contract of the Artaveo Dental Core (ADR-01, ADR-02).
//!
//! Every transport — Tauri IPC in single-PC mode, HTTPS/WebSocket in LAN mode,
//! and the plain HTTP dev server used by E2E tests — carries the same
//! [`RpcRequest`] / [`RpcResponse`] envelope and the same `method` names and
//! parameter/result types defined here. The TypeScript client types in
//! `shared/ts/contract.ts` are generated from this file (`cargo test -p
//! artaveo-shared`); CI fails if they are stale.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ───────────────────────────── envelope ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RpcRequest {
    pub method: String,
    #[serde(default)]
    #[ts(type = "unknown")]
    pub params: serde_json::Value,
    /// Session token from `auth.login`; absent for public methods.
    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RpcResponse {
    Ok {
        #[ts(type = "unknown")]
        result: serde_json::Value,
    },
    Error {
        error: RpcError,
    },
}

/// Errors carry a stable `code` the UI translates (DoD rule 4: no UI text in Core).
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct RpcError {
    pub code: ErrorCode,
    /// Developer-facing detail (English, logged); never shown verbatim to users.
    pub detail: String,
    /// The request parameter at fault (e.g. `username`, `owner_password`), so
    /// the UI shows the message under that exact input (OF-002). `None` for
    /// errors that are not about one field.
    #[serde(default)]
    pub field: Option<String>,
    /// Which rule failed; the UI translates it (`error.rule.<rule>`).
    #[serde(default)]
    pub rule: Option<ValidationRule>,
}

/// Stable, translatable reasons for a `validation` (or field-specific) error.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationRule {
    Required,
    UsernameFormat,
    UsernameTaken,
    PasswordTooShort,
    WrongPassword,
    DisplayNameLength,
    ClinicNameLength,
    RoleNotAssignable,
    OwnerImmutable,
    ColorFormat,
    TimeFormat,
    WorkingHours,
    SessionTimeoutRange,
    BackupHourRange,
    BackupKeepRange,
    LogoType,
    LogoSize,
    RecoveryKey,
    InvalidParams,
    FullNameLength,
    PhoneFormat,
    PatientNotFound,
    DateFormat,
    AgeRange,
    CannotMergeSelf,
    AttachmentType,
    AttachmentSize,
    ImportFileType,
    ToothFormat,
    PossibleDuplicate,
    // Phase 4 — staff, doctors, appointments, recalls
    DoctorNotFound,
    DoctorInactive,
    ChairNotFound,
    ChairInactive,
    ChairNotAllowed,
    ChairNameLength,
    ChairNameTaken,
    SpecialtyLength,
    UserAlreadyDoctor,
    UserNotFound,
    AppointmentNotFound,
    RecallNotFound,
    PatientMerged,
    TimeRange,
    DurationRange,
    DoctorBusy,
    ChairBusy,
    PatientBusy,
    OutsideWorkingHours,
    DoctorOnBreak,
    DoctorOnLeave,
    ScheduleOverlap,
    LeaveRange,
    InvalidTransition,
    NotEditable,
    RepeatMonthsRange,
    RoleLabelLength,
    RoleLabelTaken,
    RoleNotFound,
    RoleSystem,
    RoleInUse,
    PermissionUnknown,
    PermissionNotAllowed,
    PermissionsEmpty,
    SelfLockout,
    NotToday,
    PatientHasOpenAppointments,
    // Phase 4 — Excel/CSV import with column mapping
    ImportFileRead,
    ImportNoNameColumn,
    ImportColumn,
    // OF-036: a new booking must be for a future time.
    AppointmentInPast,
    // OF-044: calendar drag step.
    CalendarSnapRange,
    // M1 / OF-025: structured medical history.
    MedicalQuestionNotFound,
    MedicalQuestionGroup,
    MedicalQuestionLabel,
    MedicalDetailKind,
    MedicalDetailLength,
    MedicalChoice,
    MedicalMonthsRange,
    MedicalSystemQuestion,
    // Phase 5A: documents, prescriptions, formulary and templates.
    DocumentNotFound,
    DocumentVoid,
    DocumentText,
    DocumentItems,
    DoctorRequired,
    RxForm,
    RxDoseRange,
    RxWarningNotAcknowledged,
    ImagingTestUnknown,
    RestDaysRange,
    DrugNotFound,
    DrugClassUnknown,
    TemplateNotFound,
    TemplateKind,
    // M2 / M3: specialties and the service catalog.
    SpecialtyNotFound,
    ServiceNotFound,
    ServiceCategoryNotFound,
    ServiceCodeTaken,
    ServiceCodeFormat,
    ServicePrice,
    ServiceSessions,
    ServiceVariantDepth,
    ServiceSpecialty,
    LicenseLength,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotSetUp,
    AlreadySetUp,
    Unauthenticated,
    SessionExpired,
    SessionLocked,
    Forbidden,
    InvalidCredentials,
    AccountLocked,
    RecoveryKeyInvalid,
    Validation,
    /// Optimistic-locking conflict: "this record was changed by another user".
    Conflict,
    NotFound,
    UnknownMethod,
    Internal,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct Empty {}

// ───────────────────────────── app / setup ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppState {
    NeedsSetup,
    Ready,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Fa,
    Ps,
    En,
}

impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Language::Fa => "fa",
            Language::Ps => "ps",
            Language::En => "en",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AppStatus {
    pub state: AppState,
    pub version: String,
    pub environment: String,
    pub clinic_name: Option<String>,
    pub default_language: Language,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalendarSystem {
    #[default]
    Shamsi,
    Gregorian,
}

/// Roadmap 2.6: a clinic with one doctor gets a simplified UI (doctor picker
/// hidden, no per-doctor comparisons). Architecture and data are the same
/// either way; this is only a UI preference, changeable at any time.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClinicMode {
    #[default]
    Solo,
    Multi,
}

/// Only `single` is functional in Phase 2; `server`/`client` are stored
/// preferences the wizard offers but LAN mode itself is Phase 7 (ADR-01/02).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallMode {
    #[default]
    Single,
    Server,
    Client,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    Light,
    Dark,
    #[default]
    System,
}

/// One day of the clinic's working hours. `day`: 0 = Saturday … 6 = Friday
/// (the Afghan week, ADR-21). Closed days carry `open`/`close` as `None`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DayHours {
    pub day: u8,
    pub closed: bool,
    /// "HH:MM", 24-hour.
    pub open: Option<String>,
    pub close: Option<String>,
}

/// The clinic's identity and preferences (roadmap 2.5). `name`,
/// `default_language`, `logo_path` and `install_mode` are set once at Setup;
/// `clinic.update` leaves them unchanged regardless of what is sent (LAN
/// server/client conversion is Phase 7, rename/logo change is a later
/// Settings enhancement).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ClinicProfile {
    pub name: String,
    pub default_language: Language,
    pub logo_path: Option<String>,
    pub province_id: Option<String>,
    pub district_id: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub calendar_system: CalendarSystem,
    pub clinic_mode: ClinicMode,
    pub install_mode: InstallMode,
    pub theme: ThemePreference,
    pub color_primary: String,
    pub color_secondary: String,
    pub color_accent: String,
    pub working_hours: Vec<DayHours>,
    /// Set once, the first time the wizard's Trial step is acknowledged.
    /// No license enforcement yet — the real system is Phase 10.
    pub trial_started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SetupParams {
    pub clinic_name: String,
    pub owner_username: String,
    pub owner_display_name: String,
    pub owner_password: String,
    pub language: Language,
    #[serde(default)]
    pub install_mode: InstallMode,
    #[serde(default)]
    pub province_id: Option<String>,
    #[serde(default)]
    pub district_id: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    /// Base64-encoded logo image (png/jpg/webp), optional.
    #[serde(default)]
    pub logo_base64: Option<String>,
    #[serde(default)]
    pub logo_file_name: Option<String>,
    #[serde(default)]
    pub calendar_system: CalendarSystem,
    #[serde(default)]
    pub clinic_mode: ClinicMode,
    #[serde(default)]
    pub theme: ThemePreference,
    #[serde(default = "default_color_primary")]
    pub color_primary: String,
    #[serde(default = "default_color_secondary")]
    pub color_secondary: String,
    #[serde(default = "default_color_accent")]
    pub color_accent: String,
    #[serde(default)]
    pub working_hours: Vec<DayHours>,
    #[serde(default)]
    pub trial_acknowledged: bool,
}

fn default_color_primary() -> String {
    "#0e7490".into()
}
fn default_color_secondary() -> String {
    "#64748b".into()
}
fn default_color_accent() -> String {
    "#f59e0b".into()
}

/// The clinic's own logo for the header, login and splash (roadmap 2.1b).
/// `data_url` is a `data:image/...;base64,` URL, or `None` when the clinic has
/// no logo (the UI then shows the name's first letter in the clinic colour).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ClinicLogo {
    pub data_url: Option<String>,
}

/// Replaces (or with `logo_base64: None`, removes) the clinic logo.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SetLogoParams {
    #[serde(default)]
    pub logo_base64: Option<String>,
    #[serde(default)]
    pub logo_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SetupResult {
    /// Shown once; the Owner must print or write it down (ADR-04).
    pub recovery_key: String,
}

// ───────────────────────────── identity ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LoginParams {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    /// Name of a clinic-made role; built-in roles have none (the UI translates `role`).
    pub role_label: Option<String>,
    pub is_active: bool,
    pub version: i64,
    // Locked after too many wrong passwords, until the time in `locked_until` passes or an owner unlocks it.
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SessionInfo {
    pub token: String,
    pub user: UserInfo,
    pub permissions: Vec<String>,
    pub timeout_minutes: u32,
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SessionState {
    pub locked: bool,
    /// Seconds of inactivity left before the screen locks.
    pub idle_seconds_left: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UnlockParams {
    pub password: String,
}

/// The UI's activity heartbeat (OF-008/OF-012). Only this counts as user
/// activity for the idle lock; other requests (background refreshes) do not.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TouchParams {
    /// How long ago the user last moved the mouse, scrolled or typed, so the
    /// idle timer is exact although heartbeats are sent only every few seconds.
    #[serde(default)]
    pub idle_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ChangePasswordParams {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RecoverOwnerParams {
    pub recovery_key: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateUserParams {
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateUserParams {
    pub id: String,
    /// The `version` the client last saw (optimistic locking, Data Conventions 1.4).
    pub version: i64,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RoleInfo {
    pub code: String,
    /// The name a clinic gave a custom role; system roles have none (the UI translates their code).
    pub label: Option<String>,
    pub is_system: bool,
    pub permissions: Vec<String>,
    /// Active users currently holding the role.
    pub user_count: u32,
    pub version: i64,
    // A built-in role the clinic has changed; "back to defaults" clears it (OF-028).
    pub customized: bool,
}

/// A permission code the UI can show in the role editor (4.1).
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct PermissionInfo {
    pub code: String,
    /// False for permissions only the built-in roles carry (e.g. `backup.restore`, Owner-only).
    pub assignable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateRoleParams {
    pub label: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateRoleParams {
    pub code: String,
    pub version: i64,
    pub label: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DeleteRoleParams {
    pub code: String,
    pub version: i64,
}

/// Administrator sets a new password for another user (ends their sessions, clears lockout).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResetPasswordParams {
    pub id: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UserIdParams {
    pub id: String,
}

// ───────────────────────────── reference data ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReferenceListParams {
    pub type_code: String,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DistrictListParams {
    pub province_id: String,
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LanguageParams {
    pub language: Language,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct LabeledItem {
    pub id: String,
    pub code: String,
    pub label: String,
}

// ───────────────────────────── patients & medical records (Phase 3) ─────────────────────────────

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PatientStatus {
    #[default]
    Active,
    Inactive,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct PatientInfo {
    pub id: String,
    pub patient_number: String,
    pub full_name: String,
    pub father_name: Option<String>,
    pub preferred_language: Option<Language>,
    pub gender_id: Option<String>,
    pub date_of_birth: Option<String>,
    pub approximate_age: Option<i64>,
    pub phone: Option<String>,
    pub secondary_phone: Option<String>,
    pub province_id: Option<String>,
    pub district_id: Option<String>,
    pub address: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_relationship_id: Option<String>,
    pub referral_source_id: Option<String>,
    pub notes: Option<String>,
    pub registration_date: String,
    pub status: PatientStatus,
    pub merged_into_id: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct CreatePatientParams {
    pub full_name: String,
    #[serde(default)]
    pub father_name: Option<String>,
    #[serde(default)]
    pub preferred_language: Option<Language>,
    #[serde(default)]
    pub gender_id: Option<String>,
    #[serde(default)]
    pub date_of_birth: Option<String>,
    #[serde(default)]
    pub approximate_age: Option<i64>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub secondary_phone: Option<String>,
    #[serde(default)]
    pub province_id: Option<String>,
    #[serde(default)]
    pub district_id: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub emergency_contact_name: Option<String>,
    #[serde(default)]
    pub emergency_contact_phone: Option<String>,
    #[serde(default)]
    pub emergency_contact_relationship_id: Option<String>,
    #[serde(default)]
    pub referral_source_id: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub registration_date: Option<String>,
    #[serde(default)]
    pub allow_duplicate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdatePatientParams {
    pub id: String,
    pub version: i64,
    pub full_name: String,
    #[serde(default)]
    pub father_name: Option<String>,
    #[serde(default)]
    pub preferred_language: Option<Language>,
    #[serde(default)]
    pub gender_id: Option<String>,
    #[serde(default)]
    pub date_of_birth: Option<String>,
    #[serde(default)]
    pub approximate_age: Option<i64>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub secondary_phone: Option<String>,
    #[serde(default)]
    pub province_id: Option<String>,
    #[serde(default)]
    pub district_id: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub emergency_contact_name: Option<String>,
    #[serde(default)]
    pub emergency_contact_phone: Option<String>,
    #[serde(default)]
    pub emergency_contact_relationship_id: Option<String>,
    #[serde(default)]
    pub referral_source_id: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub status: PatientStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DuplicateCheckParams {
    pub full_name: String,
    #[serde(default)]
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PatientListParams {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub status: Option<PatientStatus>,
    #[serde(default = "default_patient_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_patient_limit() -> u32 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PatientListResult {
    pub items: Vec<PatientInfo>,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PatientIdParams {
    pub patient_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MergePatientsParams {
    pub keep_id: String,
    pub merge_id: String,
    pub merge_id_version: i64,
}

/// The same text in the three interface languages (clinic-editable reference data, e.g. a checklist
/// question or a service name). The Core fills an empty language from the first one given.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct Translations {
    pub fa: String,
    pub ps: String,
    pub en: String,
}

/// What a "yes" to a checklist question asks for next (M1).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MedicalDetailKind {
    /// Nothing more.
    None,
    /// A short text (a medicine's name, the kind of heart disease, …).
    Text,
    /// One of the question's `choices` (controlled / uncontrolled, …).
    Choice,
    /// A short text and one of the `choices` (a medicine allergy: name + by mouth / injected).
    TextChoice,
    /// A number of months (1–10): how far along a pregnancy is.
    Months,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MedicalAnswerValue {
    Yes,
    No,
    Unknown,
}

/// One question of the medical-history checklist (M1). System questions come from the seed and can
/// only be switched off; the clinic can add its own.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MedicalQuestionInfo {
    pub id: String,
    /// Stable code (`anticoagulants`, `pregnant`, …); prescriptions check these (5.9).
    pub code: String,
    /// `cardio`, `blood`, `endocrine`, `respiratory`, `infectious`, `kidney_liver`, `neuro`, `bone`,
    /// `cancer`, `digestive`, `skin`, `habits`, `women`, `allergy`, `medication`, `surgery`, `other`.
    pub group_code: String,
    pub sort_order: i64,
    pub detail_kind: MedicalDetailKind,
    /// Choice codes for `choice` / `text_choice` questions (`controlled`, `uncontrolled`, …).
    pub choices: Vec<String>,
    /// A "yes" shows in the medical alert banner.
    pub alert: bool,
    /// Asked only of women (pregnancy, breastfeeding).
    pub female_only: bool,
    pub is_system: bool,
    pub is_active: bool,
    pub label: Translations,
    pub detail_label: Option<Translations>,
    /// Why the alert matters, shown next to it ("antibiotic cover may be needed").
    pub alert_note: Option<Translations>,
    pub version: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct MedicalQuestionListParams {
    #[serde(default)]
    pub include_inactive: bool,
}

/// A question the clinic adds. Its detail can only be nothing or a short text.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateMedicalQuestionParams {
    pub group_code: String,
    pub label: Translations,
    #[serde(default = "detail_none")]
    pub detail_kind: MedicalDetailKind,
    #[serde(default)]
    pub alert: bool,
    #[serde(default)]
    pub female_only: bool,
}

fn detail_none() -> MedicalDetailKind {
    MedicalDetailKind::None
}

/// System questions accept only `is_active` and `sort_order`; the clinic's own accept everything.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateMedicalQuestionParams {
    pub id: String,
    pub version: i64,
    pub group_code: String,
    pub label: Translations,
    pub detail_kind: MedicalDetailKind,
    pub alert: bool,
    pub female_only: bool,
    pub is_active: bool,
    pub sort_order: i64,
}

/// A patient's answer to one checklist question. No answer at all = not asked yet.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct MedicalAnswer {
    pub question_id: String,
    pub answer: MedicalAnswerValue,
    /// Only kept for a "yes": the text detail, or the number of months for `months`.
    #[serde(default)]
    pub detail_text: Option<String>,
    /// Only kept for a "yes" to a `choice` / `text_choice` question: one of its choice codes.
    #[serde(default)]
    pub detail_choice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MedicalHistoryInfo {
    pub patient_id: String,
    /// Answers in checklist order (only questions that were asked).
    pub answers: Vec<MedicalAnswer>,
    /// «سایر توضیحات» — free text after the checklist (earlier free-text fields were moved here).
    pub notes: Option<String>,
    /// When the history was last saved or confirmed as still true, and by whom.
    pub reviewed_at: Option<String>,
    pub reviewed_by_name: Option<String>,
    /// Never recorded, or not reviewed for more than six months: ask again at the next visit.
    pub review_due: bool,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateMedicalHistoryParams {
    pub patient_id: String,
    pub version: i64,
    /// The full set of answers: a question left out becomes "not asked".
    #[serde(default)]
    pub answers: Vec<MedicalAnswer>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// "Asked again today, nothing changed": marks the history reviewed without editing it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReviewMedicalHistoryParams {
    pub patient_id: String,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Xray,
    Photo,
    Document,
    Scan,
    ConsentForm,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AttachmentInfo {
    pub id: String,
    pub patient_id: String,
    pub kind: AttachmentKind,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub tooth: Option<String>,
    pub description: Option<String>,
    pub has_thumbnail: bool,
    pub captured_at: String,
    pub created_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UploadAttachmentParams {
    pub patient_id: String,
    pub kind: AttachmentKind,
    pub file_name: String,
    pub data_base64: String,
    #[serde(default)]
    pub tooth: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub captured_at: Option<String>,
}

/// Generic id+version for optimistic-locked delete calls (`patients.delete`, `attachments.delete`).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IdVersionParams {
    pub id: String,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IdParams {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AttachmentData {
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AttachmentFileParams {
    pub id: String,
    #[serde(default)]
    pub thumbnail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportError {
    pub row_number: u32,
    /// Developer-facing detail (English).
    pub message: String,
    /// The patient field at fault and the rule it broke; the UI translates `rule.<rule>`.
    #[serde(default)]
    pub field: Option<ImportField>,
    #[serde(default)]
    pub rule: Option<ValidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportPreviewRow {
    pub row_number: u32,
    pub full_name: String,
    pub father_name: Option<String>,
    pub phone: Option<String>,
    pub errors: Vec<ImportError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportPatientsParams {
    /// CSV (UTF-8) or Excel (.xlsx) file, base64.
    pub file_base64: String,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub sheet: Option<String>,
    /// File column → patient field. Absent = use the automatic guess.
    #[serde(default)]
    pub mapping: Option<Vec<ColumnMapping>>,
    #[serde(default)]
    pub commit: bool,
    // Whether the first row holds column titles. Absent = detect it (OF-042).
    #[serde(default)]
    pub has_header: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportPatientsResult {
    pub total: u32,
    pub imported: u32,
    pub skipped: u32,
    pub preview: Vec<ImportPreviewRow>,
    pub errors: Vec<ImportError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ExportResult {
    // Full path of the saved .xlsx (inside the data folder's `exports` directory).
    pub file_path: String,
    pub file_name: String,
    pub rows: u32,
}

// ───────────────────────────── doctors, chairs, schedules (Phase 4.2) ─────────────────────────────

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActiveStatus {
    #[default]
    Active,
    Inactive,
}

/// One interval of a doctor's week. `day`: 0 = Saturday … 6 = Friday (ADR-21).
/// `start`/`end` are "HH:MM", 24-hour (the UI shows 12-hour, OF-007).
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct ScheduleSlot {
    pub day: u8,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct LeaveInfo {
    pub id: String,
    pub doctor_id: String,
    /// Clinic-local ISO dates, both ends inclusive.
    pub start_date: String,
    pub end_date: String,
    pub reason: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct DoctorInfo {
    pub id: String,
    /// The login this doctor signs in with, if any.
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub full_name: String,
    /// Free-text specialty from before v0.5.0 that matched none of the specialty list (shown as is).
    pub specialty: Option<String>,
    /// M2: the doctor's specialties (reference data), one or more.
    pub specialty_ids: Vec<String>,
    /// Licence / registration number printed on prescriptions (5.9).
    pub license_number: Option<String>,
    /// Calendar colour, `#rrggbb`.
    pub color: String,
    pub status: ActiveStatus,
    pub sort_order: i64,
    /// Weekly working hours. No intervals at all = no hour restriction.
    pub hours: Vec<ScheduleSlot>,
    pub breaks: Vec<ScheduleSlot>,
    /// Chairs this doctor may use; empty = any chair.
    pub chair_ids: Vec<String>,
    pub leaves: Vec<LeaveInfo>,
    pub version: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct DoctorListParams {
    #[serde(default)]
    pub include_inactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateDoctorParams {
    pub full_name: String,
    #[serde(default)]
    pub specialty: Option<String>,
    #[serde(default)]
    pub specialty_ids: Vec<String>,
    #[serde(default)]
    pub license_number: Option<String>,
    pub color: String,
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateDoctorParams {
    pub id: String,
    pub version: i64,
    pub full_name: String,
    #[serde(default)]
    pub specialty: Option<String>,
    #[serde(default)]
    pub specialty_ids: Vec<String>,
    #[serde(default)]
    pub license_number: Option<String>,
    pub color: String,
    pub status: ActiveStatus,
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SetScheduleParams {
    pub doctor_id: String,
    pub version: i64,
    #[serde(default)]
    pub hours: Vec<ScheduleSlot>,
    #[serde(default)]
    pub breaks: Vec<ScheduleSlot>,
    #[serde(default)]
    pub chair_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AddLeaveParams {
    pub doctor_id: String,
    pub start_date: String,
    pub end_date: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LeaveResult {
    pub leave: LeaveInfo,
    /// Live appointments of this doctor that fall inside the leave and need rescheduling.
    pub affected_appointments: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct ChairInfo {
    pub id: String,
    pub name: String,
    pub status: ActiveStatus,
    pub sort_order: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct ChairListParams {
    #[serde(default)]
    pub include_inactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateChairParams {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateChairParams {
    pub id: String,
    pub version: i64,
    pub name: String,
    pub status: ActiveStatus,
}

// ───────────────────────────── appointments & queue (Phase 4.3–4.5) ─────────────────────────────

/// ```text
/// Scheduled → Confirmed → Checked-in → In Treatment → Completed
///                     ↘ Cancelled · No-show · Rescheduled
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppointmentStatus {
    Scheduled,
    Confirmed,
    CheckedIn,
    InTreatment,
    Completed,
    Cancelled,
    NoShow,
    Rescheduled,
}

impl AppointmentStatus {
    pub fn code(self) -> &'static str {
        match self {
            AppointmentStatus::Scheduled => "scheduled",
            AppointmentStatus::Confirmed => "confirmed",
            AppointmentStatus::CheckedIn => "checked_in",
            AppointmentStatus::InTreatment => "in_treatment",
            AppointmentStatus::Completed => "completed",
            AppointmentStatus::Cancelled => "cancelled",
            AppointmentStatus::NoShow => "no_show",
            AppointmentStatus::Rescheduled => "rescheduled",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Some(match s {
            "scheduled" => AppointmentStatus::Scheduled,
            "confirmed" => AppointmentStatus::Confirmed,
            "checked_in" => AppointmentStatus::CheckedIn,
            "in_treatment" => AppointmentStatus::InTreatment,
            "completed" => AppointmentStatus::Completed,
            "cancelled" => AppointmentStatus::Cancelled,
            "no_show" => AppointmentStatus::NoShow,
            "rescheduled" => AppointmentStatus::Rescheduled,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct AppointmentInfo {
    pub id: String,
    pub patient_id: String,
    pub patient_number: String,
    pub patient_name: String,
    pub patient_phone: Option<String>,
    pub doctor_id: String,
    pub doctor_name: String,
    pub doctor_color: String,
    pub chair_id: Option<String>,
    pub chair_name: Option<String>,
    /// Clinic-local date (ISO) and 24-hour "HH:MM" start/end; the UI formats them 12-hour.
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    /// The same instants in UTC (ADR-07).
    pub start_at: String,
    pub end_at: String,
    pub reason: Option<String>,
    pub notes: Option<String>,
    /// M2/M3: the catalog service this visit is for, if chosen.
    pub service_id: Option<String>,
    /// M1: the patient's medical history was never recorded or is due for review (ask at this visit).
    pub medical_review_due: bool,
    pub status: AppointmentStatus,
    pub is_walk_in: bool,
    /// Ticket of the day, assigned at check-in.
    pub queue_number: Option<i64>,
    pub checked_in_at: Option<String>,
    pub treatment_started_at: Option<String>,
    pub completed_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub cancel_reason: Option<String>,
    pub rescheduled_from_id: Option<String>,
    pub rescheduled_to_id: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateAppointmentParams {
    pub patient_id: String,
    pub doctor_id: String,
    #[serde(default)]
    pub chair_id: Option<String>,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Book outside the doctor's hours / during a break / on a leave day anyway.
    #[serde(default)]
    pub override_schedule: bool,
    /// Booking made from the recall list (4.6): the recall becomes `booked`.
    #[serde(default)]
    pub recall_id: Option<String>,
    /// M2/M3: the catalog service the visit is for (optional).
    #[serde(default)]
    pub service_id: Option<String>,
}

/// Edits an appointment's details and/or moves it (drag & drop uses the same call).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateAppointmentParams {
    pub id: String,
    pub version: i64,
    pub doctor_id: String,
    #[serde(default)]
    pub chair_id: Option<String>,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub override_schedule: bool,
    /// Full-replace like the rest of this call: the UI always resends the current service.
    #[serde(default)]
    pub service_id: Option<String>,
}

/// Moves an appointment to a new slot, keeping the old one as `rescheduled` and linked (4.4).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RescheduleAppointmentParams {
    pub id: String,
    pub version: i64,
    pub doctor_id: String,
    #[serde(default)]
    pub chair_id: Option<String>,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    #[serde(default)]
    pub override_schedule: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct AppointmentListParams {
    /// Clinic-local ISO dates, both ends inclusive. Required unless `patient_id` is given.
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    #[serde(default)]
    pub doctor_id: Option<String>,
    #[serde(default)]
    pub chair_id: Option<String>,
    #[serde(default)]
    pub patient_id: Option<String>,
    /// Empty = every status.
    #[serde(default)]
    pub statuses: Vec<AppointmentStatus>,
    #[serde(default = "default_appointment_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_appointment_limit() -> u32 {
    500
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AppointmentCountsParams {
    pub date_from: String,
    pub date_to: String,
    #[serde(default)]
    pub doctor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct DayCount {
    pub date: String,
    /// Every appointment of the day except cancelled / rescheduled ones.
    pub total: u32,
    /// Not yet finished (scheduled, confirmed, checked-in, in treatment).
    pub open: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SetAppointmentStatusParams {
    pub id: String,
    pub version: i64,
    pub status: AppointmentStatus,
    /// Why it was cancelled (optional).
    #[serde(default)]
    pub reason: Option<String>,
    /// With `completed`: schedule the patient's next visit / recall in the same step (4.6).
    #[serde(default)]
    pub follow_up: Option<FollowUpInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WalkInParams {
    pub patient_id: String,
    pub doctor_id: String,
    #[serde(default)]
    pub chair_id: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    /// Expected length; default 30 minutes.
    #[serde(default)]
    pub duration_minutes: Option<u32>,
    /// Admit the patient outside the doctor's working hours anyway (OF-037); audited.
    #[serde(default)]
    pub override_schedule: bool,
}

// ───────────────────────────── follow-up & recall (Phase 4.6) ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecallKind {
    Checkup,
    Cleaning,
    FollowUp,
    NoShow,
    Other,
}

impl RecallKind {
    pub fn code(self) -> &'static str {
        match self {
            RecallKind::Checkup => "checkup",
            RecallKind::Cleaning => "cleaning",
            RecallKind::FollowUp => "follow_up",
            RecallKind::NoShow => "no_show",
            RecallKind::Other => "other",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Some(match s {
            "checkup" => RecallKind::Checkup,
            "cleaning" => RecallKind::Cleaning,
            "follow_up" => RecallKind::FollowUp,
            "no_show" => RecallKind::NoShow,
            "other" => RecallKind::Other,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecallStatus {
    /// Needs a phone call.
    Pending,
    /// Called, but no visit booked yet.
    Contacted,
    Booked,
    Done,
    Dismissed,
}

impl RecallStatus {
    pub fn code(self) -> &'static str {
        match self {
            RecallStatus::Pending => "pending",
            RecallStatus::Contacted => "contacted",
            RecallStatus::Booked => "booked",
            RecallStatus::Done => "done",
            RecallStatus::Dismissed => "dismissed",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Some(match s {
            "pending" => RecallStatus::Pending,
            "contacted" => RecallStatus::Contacted,
            "booked" => RecallStatus::Booked,
            "done" => RecallStatus::Done,
            "dismissed" => RecallStatus::Dismissed,
            _ => return None,
        })
    }
}

/// "Come back around `due_date`" — a follow-up or a recurring recall (e.g. scaling every 6 months).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct FollowUpInput {
    pub due_date: String,
    pub kind: RecallKind,
    /// Recurring recall: once its visit is completed, the next one is created this many months later.
    #[serde(default)]
    pub repeat_months: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct RecallInfo {
    pub id: String,
    pub patient_id: String,
    pub patient_number: String,
    pub patient_name: String,
    pub patient_phone: Option<String>,
    pub kind: RecallKind,
    pub due_date: String,
    pub repeat_months: Option<i64>,
    pub note: Option<String>,
    pub status: RecallStatus,
    pub appointment_id: Option<String>,
    pub source_appointment_id: Option<String>,
    pub last_contacted_at: Option<String>,
    pub contact_note: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateRecallParams {
    pub patient_id: String,
    pub kind: RecallKind,
    pub due_date: String,
    #[serde(default)]
    pub repeat_months: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateRecallParams {
    pub id: String,
    pub version: i64,
    pub kind: RecallKind,
    pub due_date: String,
    #[serde(default)]
    pub repeat_months: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Manual status moves only: pending ↔ contacted, or dismissed. `booked`/`done` follow the appointments.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SetRecallStatusParams {
    pub id: String,
    pub version: i64,
    pub status: RecallStatus,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct RecallListParams {
    #[serde(default)]
    pub patient_id: Option<String>,
    /// Empty = the call list (pending + contacted).
    #[serde(default)]
    pub statuses: Vec<RecallStatus>,
    /// Clinic-local ISO dates; `due_until` lets the call list show "due within two weeks".
    #[serde(default)]
    pub due_from: Option<String>,
    #[serde(default)]
    pub due_until: Option<String>,
    #[serde(default = "default_patient_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RecallListResult {
    pub items: Vec<RecallInfo>,
    pub total: u32,
}

// ───────────────────────────── specialties & service catalog (Phase 5A: M2, M3) ─────────────────────────────

/// A dental specialty (M2): reference data — nine seeded, the clinic may add or switch off its own.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct SpecialtyInfo {
    pub id: String,
    pub code: String,
    pub label: Translations,
    pub is_system: bool,
    pub is_active: bool,
    pub sort_order: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveSpecialtyParams {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: i64,
    pub label: Translations,
    #[serde(default = "yes")]
    pub is_active: bool,
}

/// Which teeth a service is for (M3): one tooth, several, a quadrant, a jaw, the whole mouth, or none.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToothScope {
    Tooth,
    Teeth,
    Quadrant,
    Arch,
    Mouth,
    None,
}

/// First level of the catalog (M3), e.g. "Restorative", "Endodontics"; usually one specialty.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct ServiceCategoryInfo {
    pub id: String,
    pub code: String,
    pub name: Translations,
    pub specialty_id: Option<String>,
    pub sort_order: i64,
    pub is_system: bool,
    pub is_active: bool,
    pub version: i64,
}

/// A service ("Root canal") or, when `parent_id` is set, one of its variants ("Two canals"). Treatment
/// plans and treatments (5B) and invoices (Phase 6) point at one of these.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct ServiceInfo {
    pub id: String,
    /// The clinic's own short code ("END-02"), shown in lists and on estimates.
    pub code: String,
    pub category_id: String,
    pub parent_id: Option<String>,
    pub name: Translations,
    /// Default price in AFN × 100 (ADR-06); 0 = not priced yet.
    pub price: i64,
    /// The specialty that does it (defaults to its category's, M2).
    pub specialty_id: Option<String>,
    pub tooth_scope: ToothScope,
    /// A tooth surface must be chosen (fillings).
    pub needs_surface: bool,
    /// Usual number of visits (root canal 2, implant 3 …), for planning (M4).
    pub sessions: i64,
    pub lab_required: bool,
    /// Consent form suggested before this treatment (5.10) and the sheet handed out after it (5.10b).
    pub consent_template_id: Option<String>,
    pub post_op_template_id: Option<String>,
    pub sort_order: i64,
    pub is_system: bool,
    pub is_active: bool,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CatalogInfo {
    pub categories: Vec<ServiceCategoryInfo>,
    /// Services and variants together, in display order.
    pub services: Vec<ServiceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveServiceCategoryParams {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: i64,
    pub name: Translations,
    #[serde(default)]
    pub specialty_id: Option<String>,
    #[serde(default = "yes")]
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveServiceParams {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: i64,
    pub code: String,
    pub category_id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub name: Translations,
    pub price: i64,
    #[serde(default)]
    pub specialty_id: Option<String>,
    pub tooth_scope: ToothScope,
    #[serde(default)]
    pub needs_surface: bool,
    #[serde(default = "one")]
    pub sessions: i64,
    #[serde(default)]
    pub lab_required: bool,
    #[serde(default)]
    pub consent_template_id: Option<String>,
    #[serde(default)]
    pub post_op_template_id: Option<String>,
    #[serde(default = "yes")]
    pub is_active: bool,
}

fn one() -> i64 {
    1
}

// ───────────────────────────── clinical documents & print engine (Phase 5A, ADR-13) ─────────────────────────────

/// Every printed clinical document (5.9, 5.10, 5.10b) is one of these; each has its own number series
/// (`RX-1405-000001`: kind prefix, Shamsi year of issue, running number).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    /// RX — prescription (5.9).
    Prescription,
    /// CF — consent form (5.10).
    Consent,
    /// PO — after-treatment instructions for the patient.
    PostOp,
    /// RF — referral letter to another doctor or specialist.
    Referral,
    /// IR — request for an X-ray / scan / lab test.
    ImagingRequest,
    /// MC — medical certificate: the visit, and rest if needed.
    Certificate,
    /// LO — work order for the dental laboratory.
    LabOrder,
    /// MR — summary of the patient's record.
    RecordSummary,
}

/// Paper size of a document. A PDF is always exactly this size (ADR-13); how a small document is put
/// on a printer's paper is a per-computer print setting.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Paper {
    A4,
    A5,
    A6,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Issued,
    /// Withdrawn with a reason; it stays in the record, marked void, and cannot be printed again.
    Void,
}

/// When a medicine is taken relative to meals or the day (printed in the patient's language).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RxTiming {
    AfterFood,
    BeforeFood,
    WithFood,
    Morning,
    Bedtime,
}

/// One line of a prescription. The medicine name is the Latin scientific/brand name the pharmacy
/// reads; the dose is structured so it prints as a sentence in the patient's language
/// ("روزانه ۳ بار، بعد از غذا، ۵ روز").
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct RxItem {
    /// The formulary entry it came from, if any (warnings use its classes).
    #[serde(default)]
    pub drug_id: Option<String>,
    pub name: String,
    /// `tablet`, `capsule`, `syrup`, `suspension`, `mouthwash`, `gel`, `cream`, `ointment`,
    /// `injection`, `drops`, `spray`, `other`.
    pub form: String,
    #[serde(default)]
    pub strength: Option<String>,
    /// How many to dispense (a count, or "1 bottle").
    #[serde(default)]
    pub quantity: Option<String>,
    /// How much each time ("1", "½", "10 ml").
    #[serde(default)]
    pub dose: Option<String>,
    #[serde(default)]
    pub times_per_day: Option<u8>,
    #[serde(default)]
    pub timing: Option<RxTiming>,
    #[serde(default)]
    pub days: Option<u16>,
    /// Only when needed (pain), up to `times_per_day`.
    #[serde(default)]
    pub as_needed: bool,
    /// Anything else for this medicine, in the patient's language.
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct PrescriptionContent {
    pub items: Vec<RxItem>,
    #[serde(default)]
    pub notes: Option<String>,
    /// The warnings (`RxWarning::key`) the doctor has seen and accepted; every warning must be here.
    #[serde(default)]
    pub acknowledged: Vec<String>,
}

/// A document written from an editable template (consent form, after-treatment instructions): the
/// text as printed, with the patient/doctor/tooth/procedure already filled in.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct TemplateDocContent {
    #[serde(default)]
    pub template_id: Option<String>,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub procedure: Option<String>,
    #[serde(default)]
    pub teeth: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct ReferralContent {
    /// Who the patient is sent to (a doctor, clinic or specialist).
    pub to: String,
    #[serde(default)]
    pub specialty: Option<String>,
    pub reason: String,
    /// Findings and treatment so far.
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub urgent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct ImagingRequestContent {
    /// `opg`, `cbct`, `periapical`, `bitewing`, `cephalometric`, `occlusal`, `blood_cbc`,
    /// `blood_coagulation`, `blood_sugar`, `hepatitis_hiv`, `other`.
    pub tests: Vec<String>,
    #[serde(default)]
    pub teeth: Option<String>,
    /// The radiology centre or medical laboratory, if known.
    #[serde(default)]
    pub center: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct CertificateContent {
    /// The visit being certified (clinic-local ISO date).
    pub visit_date: String,
    /// Days of rest advised from `rest_from` (none = the visit only).
    #[serde(default)]
    pub rest_days: Option<u16>,
    #[serde(default)]
    pub rest_from: Option<String>,
    /// Who it is for ("for the employer", "for the school").
    #[serde(default)]
    pub addressee: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Work order for a dental laboratory (linked to the Lab Case of Phase 8 later).
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct LabOrderContent {
    #[serde(default)]
    pub lab: Option<String>,
    pub teeth: String,
    /// The work: crown, bridge, denture … (often a catalog service name).
    pub work: String,
    #[serde(default)]
    pub material: Option<String>,
    /// Tooth colour (A1, A2, B1 …).
    #[serde(default)]
    pub shade: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// A summary of the patient's record, for the patient or another doctor. The Core fills `snapshot`
/// when it is issued, so a reprint shows exactly what was handed out.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct RecordSummaryContent {
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub snapshot: Option<RecordSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct RecordSnapshot {
    /// "Yes" answers of the medical checklist, labels in the document's language, detail appended.
    pub medical: Vec<String>,
    pub medical_notes: Option<String>,
    /// Latest visits: date, doctor, status code, reason.
    pub visits: Vec<RecordVisit>,
    /// Latest prescriptions: number, date, medicine names.
    pub prescriptions: Vec<RecordPrescription>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct RecordVisit {
    pub date: String,
    pub doctor: String,
    pub status: AppointmentStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct RecordPrescription {
    pub number: String,
    pub issued_at: String,
    pub medicines: Vec<String>,
}

/// What a document says, by kind.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentContent {
    Prescription(PrescriptionContent),
    Consent(TemplateDocContent),
    PostOp(TemplateDocContent),
    Referral(ReferralContent),
    ImagingRequest(ImagingRequestContent),
    Certificate(CertificateContent),
    LabOrder(LabOrderContent),
    RecordSummary(RecordSummaryContent),
}

/// The patient as printed on a document (kept with it, so a reprint is identical).
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct DocumentPatient {
    pub name: String,
    pub number: String,
    pub father_name: Option<String>,
    /// `male` / `female` (reference code), if known.
    pub gender: Option<String>,
    /// Age in years on the day of issue, if known.
    pub age: Option<i64>,
    pub phone: Option<String>,
}

/// The doctor as printed on a document's letterhead (5.9: name, specialties, licence number).
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct DocumentDoctor {
    pub name: String,
    pub license_number: Option<String>,
    pub specialties: Vec<Translations>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DocumentInfo {
    pub id: String,
    pub number: String,
    pub kind: DocumentKind,
    pub patient_id: String,
    pub patient: DocumentPatient,
    pub doctor_id: Option<String>,
    pub doctor: Option<DocumentDoctor>,
    pub appointment_id: Option<String>,
    /// The document's own language (the patient's), not the screen's.
    pub language: Language,
    pub paper: Paper,
    pub content: DocumentContent,
    pub issued_at: String,
    pub issued_by_name: Option<String>,
    pub print_count: i64,
    pub last_printed_at: Option<String>,
    /// The signed copy scanned back in (consent forms), a patient attachment.
    pub attachment_id: Option<String>,
    pub status: DocumentStatus,
    pub void_reason: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DocumentListParams {
    pub patient_id: String,
    #[serde(default)]
    pub kind: Option<DocumentKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IssueDocumentParams {
    pub patient_id: String,
    /// Required for prescriptions (their letterhead is the doctor's); optional otherwise.
    #[serde(default)]
    pub doctor_id: Option<String>,
    #[serde(default)]
    pub appointment_id: Option<String>,
    pub language: Language,
    pub paper: Paper,
    pub content: DocumentContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MarkPrintedParams {
    pub id: String,
    /// Saved as a PDF instead of printed on paper.
    #[serde(default)]
    pub pdf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct VoidDocumentParams {
    pub id: String,
    pub version: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AttachDocumentScanParams {
    pub id: String,
    pub version: i64,
    pub attachment_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RxSeverity {
    /// An allergy or a medicine to avoid: shown in red.
    Danger,
    /// Use with care (bleeding, stomach, kidney, pregnancy …): shown in amber.
    Caution,
}

/// A clash between a medicine and the patient's medical checklist (5.9), e.g. penicillin allergy +
/// Amoxicillin. The doctor must acknowledge it (by `key`) to issue the prescription.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct RxWarning {
    /// Stable id of this warning for this prescription: `<rule>:<item index>`.
    pub key: String,
    /// What clashes (`penicillin_allergy`, `nsaid_bleeding`, …); the UI words it.
    pub rule: String,
    pub item_index: u32,
    /// The checklist question behind it and the patient's detail for it (e.g. "5" months).
    pub question_code: String,
    pub detail: Option<String>,
    pub severity: RxSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CheckPrescriptionParams {
    pub patient_id: String,
    pub items: Vec<RxItem>,
}

// ───────────────────────────── formulary & templates (Phase 5A) ─────────────────────────────

/// A medicine of the clinic's formulary (5.9), seeded from common dental medicines of the essential
/// medicines list; the clinic edits it. Its defaults fill a prescription line.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct DrugInfo {
    pub id: String,
    pub code: String,
    pub name: String,
    pub form: String,
    pub strength: Option<String>,
    /// Medicine classes the warnings check: `penicillin`, `cephalosporin`, `nsaid`, `tetracycline`,
    /// `metronidazole`, `macrolide`, `lincosamide`, `opioid`, `azole`, `paracetamol`, `steroid`,
    /// `local_anesthetic`, `antiseptic`, `antifungal`, `antiviral`, `ppi`.
    pub classes: Vec<String>,
    pub quantity: Option<String>,
    pub dose: Option<String>,
    pub times_per_day: Option<u8>,
    pub timing: Option<RxTiming>,
    pub days: Option<u16>,
    pub as_needed: bool,
    pub is_system: bool,
    pub is_active: bool,
    pub version: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct IncludeInactiveParams {
    #[serde(default)]
    pub include_inactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveDrugParams {
    /// None = a new medicine.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: i64,
    pub name: String,
    pub form: String,
    #[serde(default)]
    pub strength: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub quantity: Option<String>,
    #[serde(default)]
    pub dose: Option<String>,
    #[serde(default)]
    pub times_per_day: Option<u8>,
    #[serde(default)]
    pub timing: Option<RxTiming>,
    #[serde(default)]
    pub days: Option<u16>,
    #[serde(default)]
    pub as_needed: bool,
    #[serde(default = "yes")]
    pub is_active: bool,
}

fn yes() -> bool {
    true
}

/// A ready-made prescription ("after an extraction", "tooth infection") filled in with one click.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct RxTemplateInfo {
    pub id: String,
    pub code: String,
    pub name: Translations,
    pub items: Vec<RxItem>,
    pub is_system: bool,
    pub is_active: bool,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveRxTemplateParams {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: i64,
    pub name: Translations,
    pub items: Vec<RxItem>,
    #[serde(default = "yes")]
    pub is_active: bool,
}

/// An editable text template for consent forms and after-treatment instructions (5.10, 5.10b), in
/// the three languages. Placeholders: `{patient}`, `{doctor}`, `{clinic}`, `{date}`, `{procedure}`,
/// `{teeth}`. The clinic may rewrite a built-in one (audited) and put it back to the default.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct DocumentTemplateInfo {
    pub id: String,
    pub kind: DocumentKind,
    pub code: String,
    pub title: Translations,
    pub body: Translations,
    /// Paper the document prints on by default.
    pub paper: Paper,
    pub is_system: bool,
    /// A built-in template the clinic has rewritten.
    pub customized: bool,
    pub is_active: bool,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DocumentTemplateListParams {
    #[serde(default)]
    pub kind: Option<DocumentKind>,
    #[serde(default)]
    pub include_inactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveDocumentTemplateParams {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: i64,
    pub kind: DocumentKind,
    pub title: Translations,
    pub body: Translations,
    pub paper: Paper,
    #[serde(default = "yes")]
    pub is_active: bool,
}

// ───────────────────────────── form drafts (OF-020) ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DraftParams {
    pub form_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveDraftParams {
    pub form_key: String,
    // The form's values as a JSON object, kept until the form is saved or cancelled.
    pub data_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DraftInfo {
    pub data_json: Option<String>,
    pub updated_at: Option<String>,
}

// ───────────────────────────── import with column mapping (OF-017) ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImportField {
    FullName,
    FatherName,
    Phone,
    SecondaryPhone,
    DateOfBirth,
    ApproximateAge,
    Gender,
    Province,
    Address,
    EmergencyContactName,
    EmergencyContactPhone,
    Notes,
    RegistrationDate,
}

/// File column (0-based) → patient field.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct ColumnMapping {
    pub column: u32,
    pub field: ImportField,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportInspectParams {
    pub file_base64: String,
    pub file_name: String,
    /// Sheet of an Excel file; the first sheet when absent.
    #[serde(default)]
    pub sheet: Option<String>,
    // Whether the first row holds column titles. Absent = detect it (OF-042).
    #[serde(default)]
    pub has_header: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportInspectResult {
    /// "csv" or "xlsx".
    pub file_kind: String,
    pub sheets: Vec<String>,
    pub sheet: Option<String>,
    pub headers: Vec<String>,
    /// The first rows below the header, for the mapping screen.
    pub sample_rows: Vec<Vec<String>>,
    pub total_rows: u32,
    /// Automatic guess from the header names (Dari, Pashto and English).
    pub suggested: Vec<ColumnMapping>,
    // Whether the first row was read as column titles (detected or chosen); the UI offers to flip it.
    pub has_header: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AttachmentThumbnail {
    pub id: String,
    /// `data:image/jpeg;base64,…`, or `None` when the file is not an image the Core can shrink.
    pub data_url: Option<String>,
}

// ───────────────────────────── backup / audit / system ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BackupInfo {
    pub id: String,
    pub file_name: String,
    pub created_at: String,
    pub size_bytes: i64,
    /// "manual" | "daily" | "pre_migration"
    pub kind: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AuditListParams {
    pub limit: u32,
    pub offset: u32,
    /// Narrows to one record's history (e.g. a patient's Audit History tab, 3.6).
    #[serde(default)]
    pub entity_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AuditEntry {
    pub id: i64,
    pub at: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub action: String,
    pub entity: Option<String>,
    pub entity_id: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub computer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Settings {
    pub session_timeout_minutes: u32,
    pub daily_backup_hour: u32,
    pub backup_keep_daily: u32,
    /// OF-044: an appointment dragged in the calendar lands on steps of this many minutes (1, 5, 10 or 15).
    #[serde(default = "default_calendar_snap")]
    pub calendar_snap_minutes: u32,
    /// 2.1b: a very small "Artaveo Dental" mark at the foot of printed documents (the clinic may turn it off).
    #[serde(default = "yes")]
    pub print_brand_footer: bool,
    /// M2: only doctors of a service's specialty may be booked for it (off: a gentle warning only).
    #[serde(default)]
    pub restrict_service_specialty: bool,
}

fn default_calendar_snap() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DatabaseInfo {
    pub encrypted: bool,
    pub cipher_version: String,
    pub sqlite_version: String,
    /// "windows_dpapi_machine" | "insecure_dev_file"
    pub key_protection: String,
    pub schema_version: i64,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IntegrityInfo {
    /// "pending" | "running" | "ok" | "failed"
    pub status: String,
    pub checked_at: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SystemInfo {
    pub app_version: String,
    pub git_commit: String,
    /// CPU architecture this build was compiled for: "x64" | "x86" | "arm64".
    pub build_arch: String,
    /// The machine's native architecture (differs when emulated, e.g. x86 on ARM64).
    pub machine_arch: String,
    pub emulated: bool,
    pub os: String,
    pub environment: String,
    pub computer_name: String,
    pub data_dir: String,
    pub log_dir: String,
    pub database: Option<DatabaseInfo>,
    pub integrity: IntegrityInfo,
    pub last_backup: Option<BackupInfo>,
}

// ───────────────────────────── method registry ─────────────────────────────

macro_rules! api {
    ($( $name:literal => $konst:ident ( $params:ty ) -> $result:ty ; )*) => {
        /// Method names, usable from Rust without string typos.
        pub mod methods {
            $( pub const $konst: &str = $name; )*
        }

        /// All methods, in declaration order.
        pub const ALL_METHODS: &[&str] = &[$($name),*];


        fn ts_api_map(cfg: &ts_rs::Config) -> String {
            let mut s = String::from("export interface Api {\n");
            $(
                s.push_str(&format!(
                    "  \"{}\": {{ params: {}; result: {} }};\n",
                    $name,
                    <$params as TS>::name(cfg),
                    <$result as TS>::name(cfg),
                ));
            )*
            s.push_str("}\n");
            s
        }
    };
}

api! {
    "app.status"             => APP_STATUS(Empty) -> AppStatus;
    "app.setup"              => APP_SETUP(SetupParams) -> SetupResult;
    "app.clinic_logo"        => APP_CLINIC_LOGO(Empty) -> ClinicLogo;
    "auth.login"             => AUTH_LOGIN(LoginParams) -> SessionInfo;
    "auth.recover_owner"     => AUTH_RECOVER_OWNER(RecoverOwnerParams) -> Empty;
    "auth.logout"            => AUTH_LOGOUT(Empty) -> Empty;
    "auth.change_password"   => AUTH_CHANGE_PASSWORD(ChangePasswordParams) -> Empty;
    "session.state"          => SESSION_STATE(Empty) -> SessionState;
    "session.touch"          => SESSION_TOUCH(TouchParams) -> SessionState;
    "session.lock"           => SESSION_LOCK(Empty) -> SessionState;
    "session.unlock"         => SESSION_UNLOCK(UnlockParams) -> SessionState;
    "users.list"             => USERS_LIST(Empty) -> Vec<UserInfo>;
    "users.create"           => USERS_CREATE(CreateUserParams) -> UserInfo;
    "users.update"           => USERS_UPDATE(UpdateUserParams) -> UserInfo;
    "users.reset_password"   => USERS_RESET_PASSWORD(ResetPasswordParams) -> UserInfo;
    "users.unlock"           => USERS_UNLOCK(UserIdParams) -> UserInfo;
    "roles.list"             => ROLES_LIST(Empty) -> Vec<RoleInfo>;
    "roles.create"           => ROLES_CREATE(CreateRoleParams) -> RoleInfo;
    "roles.update"           => ROLES_UPDATE(UpdateRoleParams) -> RoleInfo;
    "roles.delete"           => ROLES_DELETE(DeleteRoleParams) -> Empty;
    "permissions.list"       => PERMISSIONS_LIST(Empty) -> Vec<PermissionInfo>;
    "reference.list"         => REFERENCE_LIST(ReferenceListParams) -> Vec<LabeledItem>;
    "geo.provinces"          => GEO_PROVINCES(LanguageParams) -> Vec<LabeledItem>;
    "geo.districts"          => GEO_DISTRICTS(DistrictListParams) -> Vec<LabeledItem>;
    "clinic.get"             => CLINIC_GET(Empty) -> ClinicProfile;
    "clinic.update"          => CLINIC_UPDATE(ClinicProfile) -> ClinicProfile;
    "clinic.set_logo"        => CLINIC_SET_LOGO(SetLogoParams) -> ClinicLogo;
    "settings.get"           => SETTINGS_GET(Empty) -> Settings;
    "settings.update"        => SETTINGS_UPDATE(Settings) -> Settings;
    "patients.list"              => PATIENTS_LIST(PatientListParams) -> PatientListResult;
    "patients.get"               => PATIENTS_GET(PatientIdParams) -> PatientInfo;
    "patients.create"            => PATIENTS_CREATE(CreatePatientParams) -> PatientInfo;
    "patients.update"            => PATIENTS_UPDATE(UpdatePatientParams) -> PatientInfo;
    "patients.delete"            => PATIENTS_DELETE(IdVersionParams) -> Empty;
    "patients.check_duplicate"   => PATIENTS_CHECK_DUPLICATE(DuplicateCheckParams) -> Vec<PatientInfo>;
    "patients.merge"             => PATIENTS_MERGE(MergePatientsParams) -> PatientInfo;
    "patients.import_inspect"    => PATIENTS_IMPORT_INSPECT(ImportInspectParams) -> ImportInspectResult;
    "patients.import"            => PATIENTS_IMPORT(ImportPatientsParams) -> ImportPatientsResult;
    "patients.export"            => PATIENTS_EXPORT(Empty) -> ExportResult;
    "medical_history.get"        => MEDICAL_HISTORY_GET(PatientIdParams) -> MedicalHistoryInfo;
    "medical_history.update"     => MEDICAL_HISTORY_UPDATE(UpdateMedicalHistoryParams) -> MedicalHistoryInfo;
    "medical_history.review"     => MEDICAL_HISTORY_REVIEW(ReviewMedicalHistoryParams) -> MedicalHistoryInfo;
    "medical_questions.list"     => MEDICAL_QUESTIONS_LIST(MedicalQuestionListParams) -> Vec<MedicalQuestionInfo>;
    "medical_questions.create"   => MEDICAL_QUESTIONS_CREATE(CreateMedicalQuestionParams) -> MedicalQuestionInfo;
    "medical_questions.update"   => MEDICAL_QUESTIONS_UPDATE(UpdateMedicalQuestionParams) -> MedicalQuestionInfo;
    "attachments.list"           => ATTACHMENTS_LIST(PatientIdParams) -> Vec<AttachmentInfo>;
    "attachments.upload"         => ATTACHMENTS_UPLOAD(UploadAttachmentParams) -> AttachmentInfo;
    "attachments.delete"         => ATTACHMENTS_DELETE(IdVersionParams) -> Empty;
    "attachments.file"           => ATTACHMENTS_FILE(AttachmentFileParams) -> AttachmentData;
    "attachments.thumbnails"     => ATTACHMENTS_THUMBNAILS(PatientIdParams) -> Vec<AttachmentThumbnail>;
    "doctors.list"               => DOCTORS_LIST(DoctorListParams) -> Vec<DoctorInfo>;
    "doctors.create"             => DOCTORS_CREATE(CreateDoctorParams) -> DoctorInfo;
    "doctors.update"             => DOCTORS_UPDATE(UpdateDoctorParams) -> DoctorInfo;
    "doctors.set_schedule"       => DOCTORS_SET_SCHEDULE(SetScheduleParams) -> DoctorInfo;
    "doctors.add_leave"          => DOCTORS_ADD_LEAVE(AddLeaveParams) -> LeaveResult;
    "doctors.delete_leave"       => DOCTORS_DELETE_LEAVE(IdVersionParams) -> Empty;
    "chairs.list"                => CHAIRS_LIST(ChairListParams) -> Vec<ChairInfo>;
    "chairs.create"              => CHAIRS_CREATE(CreateChairParams) -> ChairInfo;
    "chairs.update"              => CHAIRS_UPDATE(UpdateChairParams) -> ChairInfo;
    "appointments.list"          => APPOINTMENTS_LIST(AppointmentListParams) -> Vec<AppointmentInfo>;
    "appointments.counts"        => APPOINTMENTS_COUNTS(AppointmentCountsParams) -> Vec<DayCount>;
    "appointments.get"           => APPOINTMENTS_GET(IdParams) -> AppointmentInfo;
    "appointments.create"        => APPOINTMENTS_CREATE(CreateAppointmentParams) -> AppointmentInfo;
    "appointments.update"        => APPOINTMENTS_UPDATE(UpdateAppointmentParams) -> AppointmentInfo;
    "appointments.reschedule"    => APPOINTMENTS_RESCHEDULE(RescheduleAppointmentParams) -> AppointmentInfo;
    "appointments.set_status"    => APPOINTMENTS_SET_STATUS(SetAppointmentStatusParams) -> AppointmentInfo;
    "appointments.walk_in"       => APPOINTMENTS_WALK_IN(WalkInParams) -> AppointmentInfo;
    "recalls.list"               => RECALLS_LIST(RecallListParams) -> RecallListResult;
    "recalls.create"             => RECALLS_CREATE(CreateRecallParams) -> RecallInfo;
    "recalls.update"             => RECALLS_UPDATE(UpdateRecallParams) -> RecallInfo;
    "recalls.set_status"         => RECALLS_SET_STATUS(SetRecallStatusParams) -> RecallInfo;
    "backup.create"          => BACKUP_CREATE(Empty) -> BackupInfo;
    "backup.list"            => BACKUP_LIST(Empty) -> Vec<BackupInfo>;
    "audit.list"             => AUDIT_LIST(AuditListParams) -> Vec<AuditEntry>;
    "system.info"            => SYSTEM_INFO(Empty) -> SystemInfo;
    "drafts.get"             => DRAFTS_GET(DraftParams) -> DraftInfo;
    "drafts.save"            => DRAFTS_SAVE(SaveDraftParams) -> Empty;
    "drafts.delete"          => DRAFTS_DELETE(DraftParams) -> Empty;
    "roles.reset"            => ROLES_RESET(DeleteRoleParams) -> RoleInfo;
    "documents.list"             => DOCUMENTS_LIST(DocumentListParams) -> Vec<DocumentInfo>;
    "documents.get"              => DOCUMENTS_GET(IdParams) -> DocumentInfo;
    "documents.issue"            => DOCUMENTS_ISSUE(IssueDocumentParams) -> DocumentInfo;
    "documents.mark_printed"     => DOCUMENTS_MARK_PRINTED(MarkPrintedParams) -> DocumentInfo;
    "documents.void"             => DOCUMENTS_VOID(VoidDocumentParams) -> DocumentInfo;
    "documents.attach_scan"      => DOCUMENTS_ATTACH_SCAN(AttachDocumentScanParams) -> DocumentInfo;
    "prescriptions.check"        => PRESCRIPTIONS_CHECK(CheckPrescriptionParams) -> Vec<RxWarning>;
    "drugs.list"                 => DRUGS_LIST(IncludeInactiveParams) -> Vec<DrugInfo>;
    "drugs.save"                 => DRUGS_SAVE(SaveDrugParams) -> DrugInfo;
    "rx_templates.list"          => RX_TEMPLATES_LIST(IncludeInactiveParams) -> Vec<RxTemplateInfo>;
    "rx_templates.save"          => RX_TEMPLATES_SAVE(SaveRxTemplateParams) -> RxTemplateInfo;
    "document_templates.list"    => DOCUMENT_TEMPLATES_LIST(DocumentTemplateListParams) -> Vec<DocumentTemplateInfo>;
    "document_templates.save"    => DOCUMENT_TEMPLATES_SAVE(SaveDocumentTemplateParams) -> DocumentTemplateInfo;
    "document_templates.reset"   => DOCUMENT_TEMPLATES_RESET(IdVersionParams) -> DocumentTemplateInfo;
    "specialties.list"           => SPECIALTIES_LIST(IncludeInactiveParams) -> Vec<SpecialtyInfo>;
    "specialties.save"           => SPECIALTIES_SAVE(SaveSpecialtyParams) -> SpecialtyInfo;
    "catalog.get"                => CATALOG_GET(IncludeInactiveParams) -> CatalogInfo;
    "catalog.save_category"      => CATALOG_SAVE_CATEGORY(SaveServiceCategoryParams) -> ServiceCategoryInfo;
    "catalog.save_service"       => CATALOG_SAVE_SERVICE(SaveServiceParams) -> ServiceInfo;
}

/// Methods callable without a session (status, first-run setup, the clinic
/// logo shown on the login screen, login, recovery).
pub const PUBLIC_METHODS: &[&str] = &[
    methods::APP_STATUS,
    methods::APP_SETUP,
    methods::APP_CLINIC_LOGO,
    methods::AUTH_LOGIN,
    methods::AUTH_RECOVER_OWNER,
];

/// Generates `shared/ts/contract.ts`.
pub fn typescript_bindings() -> String {
    let cfg = ts_rs::Config::new().with_large_int("number");
    let mut out = String::from(
        "// GENERATED by `cargo test -p artaveo-shared` from shared/src/lib.rs — do not edit.\n\n",
    );
    macro_rules! decl {
        ($($t:ty),* $(,)?) => { $( out.push_str(&format!("export {}\n\n", <$t as TS>::decl(&cfg))); )* };
    }
    decl!(
        RpcRequest,
        RpcResponse,
        RpcError,
        ValidationRule,
        ErrorCode,
        Empty,
        AppState,
        Language,
        AppStatus,
        CalendarSystem,
        ClinicMode,
        InstallMode,
        ThemePreference,
        DayHours,
        ClinicProfile,
        SetupParams,
        ClinicLogo,
        SetLogoParams,
        SetupResult,
        LoginParams,
        UserInfo,
        SessionInfo,
        SessionState,
        UnlockParams,
        TouchParams,
        ChangePasswordParams,
        RecoverOwnerParams,
        CreateUserParams,
        UpdateUserParams,
        RoleInfo,
        PermissionInfo,
        CreateRoleParams,
        UpdateRoleParams,
        DeleteRoleParams,
        ResetPasswordParams,
        UserIdParams,
        ReferenceListParams,
        DistrictListParams,
        LanguageParams,
        LabeledItem,
        BackupInfo,
        AuditListParams,
        AuditEntry,
        Settings,
        DatabaseInfo,
        IntegrityInfo,
        SystemInfo,
        PatientStatus,
        PatientInfo,
        CreatePatientParams,
        UpdatePatientParams,
        DuplicateCheckParams,
        PatientListParams,
        PatientListResult,
        PatientIdParams,
        MergePatientsParams,
        Translations,
        MedicalDetailKind,
        MedicalAnswerValue,
        MedicalQuestionInfo,
        MedicalQuestionListParams,
        CreateMedicalQuestionParams,
        UpdateMedicalQuestionParams,
        MedicalAnswer,
        MedicalHistoryInfo,
        UpdateMedicalHistoryParams,
        ReviewMedicalHistoryParams,
        AttachmentKind,
        AttachmentInfo,
        UploadAttachmentParams,
        IdVersionParams,
        AttachmentData,
        AttachmentFileParams,
        IdParams,
        ImportError,
        ImportPreviewRow,
        ImportPatientsParams,
        ImportPatientsResult,
        ExportResult,
        ImportField,
        ColumnMapping,
        ImportInspectParams,
        ImportInspectResult,
        AttachmentThumbnail,
        ActiveStatus,
        ScheduleSlot,
        LeaveInfo,
        DoctorInfo,
        DoctorListParams,
        CreateDoctorParams,
        UpdateDoctorParams,
        SetScheduleParams,
        AddLeaveParams,
        LeaveResult,
        ChairInfo,
        ChairListParams,
        CreateChairParams,
        UpdateChairParams,
        AppointmentStatus,
        AppointmentInfo,
        CreateAppointmentParams,
        UpdateAppointmentParams,
        RescheduleAppointmentParams,
        AppointmentListParams,
        AppointmentCountsParams,
        DayCount,
        SetAppointmentStatusParams,
        WalkInParams,
        RecallKind,
        RecallStatus,
        FollowUpInput,
        RecallInfo,
        CreateRecallParams,
        UpdateRecallParams,
        SetRecallStatusParams,
        RecallListParams,
        RecallListResult,
        DraftParams,
        SaveDraftParams,
        DraftInfo,
        DocumentKind,
        Paper,
        DocumentStatus,
        RxTiming,
        RxItem,
        PrescriptionContent,
        TemplateDocContent,
        ReferralContent,
        ImagingRequestContent,
        CertificateContent,
        LabOrderContent,
        RecordSummaryContent,
        RecordSnapshot,
        RecordVisit,
        RecordPrescription,
        DocumentContent,
        DocumentPatient,
        DocumentDoctor,
        DocumentInfo,
        DocumentListParams,
        IssueDocumentParams,
        MarkPrintedParams,
        VoidDocumentParams,
        AttachDocumentScanParams,
        RxSeverity,
        RxWarning,
        CheckPrescriptionParams,
        DrugInfo,
        IncludeInactiveParams,
        SaveDrugParams,
        RxTemplateInfo,
        SaveRxTemplateParams,
        DocumentTemplateInfo,
        DocumentTemplateListParams,
        SaveDocumentTemplateParams,
        SpecialtyInfo,
        SaveSpecialtyParams,
        ToothScope,
        ServiceCategoryInfo,
        ServiceInfo,
        CatalogInfo,
        SaveServiceCategoryParams,
        SaveServiceParams,
    );
    out.push_str(&ts_api_map(&cfg));
    out.push_str(&format!(
        "\nexport const PUBLIC_METHODS = {} as const;\n",
        serde_json::to_string(PUBLIC_METHODS).unwrap()
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regenerates the TypeScript contract. CI runs this and then
    /// `git diff --exit-code shared/ts` so stale bindings fail the build.
    #[test]
    fn export_typescript_contract() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ts/contract.ts");
        std::fs::write(&path, typescript_bindings()).unwrap();
    }

    #[test]
    fn method_names_are_unique_and_public_subset_is_correct() {
        let mut names = ALL_METHODS.to_vec();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), ALL_METHODS.len());
        assert_eq!(
            PUBLIC_METHODS,
            &["app.status", "app.setup", "app.clinic_logo", "auth.login", "auth.recover_owner"]
        );
    }

    #[test]
    fn envelope_shape() {
        let ok = RpcResponse::Ok { result: serde_json::json!({"a": 1}) };
        assert_eq!(
            serde_json::to_value(ok).unwrap(),
            serde_json::json!({"status": "ok", "result": {"a": 1}})
        );
        let err = RpcResponse::Error {
            error: RpcError {
                code: ErrorCode::Validation,
                detail: "x".into(),
                field: Some("username".into()),
                rule: Some(ValidationRule::UsernameFormat),
            },
        };
        assert_eq!(
            serde_json::to_value(err).unwrap(),
            serde_json::json!({"status": "error", "error": {"code": "validation", "detail": "x", "field": "username", "rule": "username_format"}})
        );
    }
}
