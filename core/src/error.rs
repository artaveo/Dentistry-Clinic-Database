use artaveo_shared::{ErrorCode, RpcError};

use crate::keys::KeyError;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{code:?}: {detail}")]
    Api { code: ErrorCode, detail: String },
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
        CoreError::Api { code, detail: detail.into() }
    }
    pub fn validation(detail: impl Into<String>) -> Self {
        Self::api(ErrorCode::Validation, detail)
    }
    pub fn forbidden(permission: &str) -> Self {
        Self::api(ErrorCode::Forbidden, format!("missing permission {permission}"))
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            CoreError::Api { code, .. } => *code,
            _ => ErrorCode::Internal,
        }
    }

    pub fn to_rpc(&self) -> RpcError {
        RpcError { code: self.code(), detail: self.to_string() }
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
