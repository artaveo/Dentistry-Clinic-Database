//! Dev / Test / Production configuration (roadmap 1.1).

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Test,
    Production,
}

impl Environment {
    /// `ARTAVEO_ENV=development|test|production`; otherwise debug builds are
    /// Development and release builds are Production.
    pub fn detect() -> Self {
        match std::env::var("ARTAVEO_ENV").as_deref() {
            Ok("production") => Environment::Production,
            Ok("test") => Environment::Test,
            Ok("development") => Environment::Development,
            _ if cfg!(debug_assertions) => Environment::Development,
            _ => Environment::Production,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Environment::Development => "development",
            Environment::Test => "test",
            Environment::Production => "production",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: Environment,
    /// Holds `clinic.db`, key files, `backups/` and `logs/`.
    pub data_dir: PathBuf,
}

impl Config {
    pub fn new(environment: Environment, data_dir: PathBuf) -> Self {
        Self { environment, data_dir }
    }

    /// `ARTAVEO_DATA_DIR` wins; otherwise:
    /// * Production on Windows: `%ProgramData%\ArtaveoDental` — machine-wide,
    ///   shared by every Windows account and by the future LAN service
    ///   (matches DPAPI machine scope, ADR-04). The installer grants Users
    ///   modify rights on it.
    /// * Production elsewhere: `$XDG_DATA_HOME/artaveo-dental`.
    /// * Development/Test: `./.artaveo-<env>` in the working directory.
    pub fn from_env() -> Self {
        let environment = Environment::detect();
        if let Some(dir) = std::env::var_os("ARTAVEO_DATA_DIR") {
            return Self::new(environment, PathBuf::from(dir));
        }
        let data_dir = match environment {
            Environment::Production => production_dir(),
            other => PathBuf::from(format!(".artaveo-{}", other.code())),
        };
        Self::new(environment, data_dir)
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("clinic.db")
    }
    pub fn backup_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }
    pub fn log_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }
}

fn production_dir() -> PathBuf {
    if cfg!(windows) {
        let base = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
        base.join("ArtaveoDental")
    } else {
        let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
        });
        base.join("artaveo-dental")
    }
}
