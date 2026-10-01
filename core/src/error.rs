use artaveo_shared::{ErrorCode, RpcError, ValidationRule};

use crate::keys::KeyError;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{code:?}: {detail}")]
    Api { code: ErrorCode, detail: String, field: Option<String>, rule: Option<ValidationRule> },
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Key(#[from] KeyError),
    #[error("database key is wrong or the file is not an Artaveo database")]
    WrongKey,
    #[error("integrity check failed: {0}")]
    Integrity(String),
    #[error("migration {version} was modified after being applied")]
    MigrationDrift { version: i64 },
    #[error("migration {version} failed: {source}")]
    MigrationFailed { version: i64, source: rusqlite::Error },
    #[error("database schema v{db} is newer than this app (v{app}); update the app")]
    DatabaseNewerThanApp { db: i64, app: i64 },
}

impl CoreError {
    pub fn api(code: ErrorCode, detail: impl Into<String>) -> Self {
        CoreError::Api { code, detail: detail.into(), field: None, rule: None }
    }
    /// A validation error not tied to one input (malformed request, …).
    pub fn validation(detail: impl Into<String>) -> Self {
        Self::api(ErrorCode::Validation, detail)
    }
    /// A validation error about one request field, shown under that input (OF-002).
    pub fn invalid(field: &str, rule: ValidationRule, detail: impl Into<String>) -> Self {
        CoreError::Api { code: ErrorCode::Validation, detail: detail.into(), field: Some(field.into()), rule: Some(rule) }
    }
    pub fn forbidden(permission: &str) -> Self {
        Self::api(ErrorCode::Forbidden, format!("missing permission {permission}"))
    }

    /// Attaches the request field (and rule) an error is about, keeping its code.
    pub fn on_field(self, field: &str, rule: ValidationRule) -> Self {
        match self {
            CoreError::Api { code, detail, .. } => {
                CoreError::Api { code, detail, field: Some(field.into()), rule: Some(rule) }
            }
            other => other,
        }
    }

    /// Renames the field of a field error, e.g. a shared username check
    /// reported as `owner_username` by Setup.
    pub fn rename_field(self, field: &str) -> Self {
        match self {
            CoreError::Api { code, detail, field: Some(_), rule } => {
                CoreError::Api { code, detail, field: Some(field.into()), rule }
            }
            other => other,
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            CoreError::Api { code, .. } => *code,
            _ => ErrorCode::Internal,
        }
    }

    pub fn field(&self) -> Option<&str> {
        match self {
            CoreError::Api { field, .. } => field.as_deref(),
            _ => None,
        }
    }

    pub fn to_rpc(&self) -> RpcError {
        let (field, rule) = match self {
            CoreError::Api { field, rule, .. } => (field.clone(), *rule),
            _ => (None, None),
        };
        RpcError { code: self.code(), detail: self.to_string(), field, rule }
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
