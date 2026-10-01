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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalendarSystem {
    Shamsi,
    Gregorian,
}
impl Default for CalendarSystem {
    fn default() -> Self {
        CalendarSystem::Shamsi
    }
}

/// Roadmap 2.6: a clinic with one doctor gets a simplified UI (doctor picker
/// hidden, no per-doctor comparisons). Architecture and data are the same
/// either way; this is only a UI preference, changeable at any time.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClinicMode {
    Solo,
    Multi,
}
impl Default for ClinicMode {
    fn default() -> Self {
        ClinicMode::Solo
    }
}

/// Only `single` is functional in Phase 2; `server`/`client` are stored
/// preferences the wizard offers but LAN mode itself is Phase 7 (ADR-01/02).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallMode {
    Single,
    Server,
    Client,
}
impl Default for InstallMode {
    fn default() -> Self {
        InstallMode::Single
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    Light,
    Dark,
    System,
}
impl Default for ThemePreference {
    fn default() -> Self {
        ThemePreference::System
    }
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
    pub is_active: bool,
    pub version: i64,
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
    pub permissions: Vec<String>,
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
    "auth.login"             => AUTH_LOGIN(LoginParams) -> SessionInfo;
    "auth.recover_owner"     => AUTH_RECOVER_OWNER(RecoverOwnerParams) -> Empty;
    "auth.logout"            => AUTH_LOGOUT(Empty) -> Empty;
    "auth.change_password"   => AUTH_CHANGE_PASSWORD(ChangePasswordParams) -> Empty;
    "session.state"          => SESSION_STATE(Empty) -> SessionState;
    "session.touch"          => SESSION_TOUCH(Empty) -> SessionState;
    "session.lock"           => SESSION_LOCK(Empty) -> SessionState;
    "session.unlock"         => SESSION_UNLOCK(UnlockParams) -> SessionState;
    "users.list"             => USERS_LIST(Empty) -> Vec<UserInfo>;
    "users.create"           => USERS_CREATE(CreateUserParams) -> UserInfo;
    "users.update"           => USERS_UPDATE(UpdateUserParams) -> UserInfo;
    "roles.list"             => ROLES_LIST(Empty) -> Vec<RoleInfo>;
    "reference.list"         => REFERENCE_LIST(ReferenceListParams) -> Vec<LabeledItem>;
    "geo.provinces"          => GEO_PROVINCES(LanguageParams) -> Vec<LabeledItem>;
    "geo.districts"          => GEO_DISTRICTS(DistrictListParams) -> Vec<LabeledItem>;
    "clinic.get"             => CLINIC_GET(Empty) -> ClinicProfile;
    "clinic.update"          => CLINIC_UPDATE(ClinicProfile) -> ClinicProfile;
    "settings.get"           => SETTINGS_GET(Empty) -> Settings;
    "settings.update"        => SETTINGS_UPDATE(Settings) -> Settings;
    "backup.create"          => BACKUP_CREATE(Empty) -> BackupInfo;
    "backup.list"            => BACKUP_LIST(Empty) -> Vec<BackupInfo>;
    "audit.list"             => AUDIT_LIST(AuditListParams) -> Vec<AuditEntry>;
    "system.info"            => SYSTEM_INFO(Empty) -> SystemInfo;
}

/// Methods callable without a session (status, first-run setup, login, recovery).
pub const PUBLIC_METHODS: &[&str] =
    &[methods::APP_STATUS, methods::APP_SETUP, methods::AUTH_LOGIN, methods::AUTH_RECOVER_OWNER];

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
        SetupResult,
        LoginParams,
        UserInfo,
        SessionInfo,
        SessionState,
        UnlockParams,
        ChangePasswordParams,
        RecoverOwnerParams,
        CreateUserParams,
        UpdateUserParams,
        RoleInfo,
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
        assert_eq!(PUBLIC_METHODS, &["app.status", "app.setup", "auth.login", "auth.recover_owner"]);
    }

    #[test]
    fn envelope_shape() {
        let ok = RpcResponse::Ok { result: serde_json::json!({"a": 1}) };
        assert_eq!(
            serde_json::to_value(ok).unwrap(),
            serde_json::json!({"status": "ok", "result": {"a": 1}})
        );
        let err = RpcResponse::Error { error: RpcError { code: ErrorCode::Conflict, detail: "x".into() } };
        assert_eq!(
            serde_json::to_value(err).unwrap(),
            serde_json::json!({"status": "error", "error": {"code": "conflict", "detail": "x"}})
        );
    }
}
