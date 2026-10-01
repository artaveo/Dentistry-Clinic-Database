//! Structured logging with daily rotation (roadmap 1.1).
//! JSON lines in `<data>/logs/artaveo.YYYY-MM-DD.log`, 14 files kept.
//! Never log patient data, passwords, tokens or keys.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::Environment;

pub const KEEP_LOG_FILES: usize = 14;

/// Initialise once per process; keep the returned guard alive until exit
/// (dropping it flushes the background writer).
pub fn init(log_dir: &Path, env: Environment) -> std::io::Result<WorkerGuard> {
    std::fs::create_dir_all(log_dir)?;
    let appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("artaveo")
        .filename_suffix("log")
        .max_log_files(KEEP_LOG_FILES)
        .build(log_dir)
        .map_err(std::io::Error::other)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let default_level = if env == Environment::Production { "info" } else { "debug" };
    let filter = EnvFilter::try_from_env("ARTAVEO_LOG").unwrap_or_else(|_| EnvFilter::new(default_level));
    let file_layer = fmt::layer().json().with_current_span(false).with_writer(writer);
    let console = (env != Environment::Production).then(|| fmt::layer().compact());
    // A second init (tests, dev restarts) is harmless; keep the first subscriber.
    let _ = tracing_subscriber::registry().with(filter).with(file_layer).with(console).try_init();
    Ok(guard)
}
