// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Tauri shell (ADR-01/02): owns the window and forwards every UI request to
//! the Rust Core through one IPC command. It contains no business logic.

use std::sync::Arc;
use std::time::Duration;

use artaveo_core::{Config, Core};
use artaveo_shared::{ErrorCode, RpcError, RpcRequest, RpcResponse};

/// The Core, or why it could not start (e.g. the database key is missing on
/// this computer and the clinic must be restored with its Recovery Key).
enum Backend {
    Ready(Arc<Core>),
    Failed(String),
}

#[tauri::command]
async fn rpc(state: tauri::State<'_, Arc<Backend>>, request: RpcRequest) -> Result<RpcResponse, String> {
    let backend = state.inner().clone();
    // Argon2id and backups take real CPU time: keep them off the UI thread.
    tauri::async_runtime::spawn_blocking(move || match backend.as_ref() {
        Backend::Ready(core) => core.handle(request),
        Backend::Failed(detail) => RpcResponse::Error {
            error: RpcError {
                code: ErrorCode::Internal,
                detail: format!("startup failed: {detail}"),
                field: None,
                rule: None,
            },
        },
    })
    .await
    .map_err(|e| e.to_string())
}

fn main() {
    let config = Config::from_env();
    let _log_guard = artaveo_core::logging::init(&config.log_dir(), config.environment).ok();
    tracing::info!(
        version = artaveo_core::APP_VERSION,
        commit = artaveo_core::GIT_COMMIT,
        arch = artaveo_core::sysinfo::build_arch(),
        env = config.environment.code(),
        data_dir = %config.data_dir.display(),
        "starting Artaveo Dental"
    );

    let backend = match Core::open(config) {
        Ok(core) => {
            // CI's upgrade test reads this line: an existing clinic must still open after an update.
            tracing::info!(set_up = core.is_set_up(), "core ready");
            let core = Arc::new(core);
            let tick = core.clone();
            std::thread::spawn(move || loop {
                // Daily local backup (roadmap 1.7); cheap when nothing is due.
                if let Err(e) = tick.run_due_tasks() {
                    tracing::error!(error = %e, "scheduled task failed");
                }
                std::thread::sleep(Duration::from_secs(300));
            });
            Backend::Ready(core)
        }
        Err(e) => {
            tracing::error!(error = %e, "core failed to start");
            Backend::Failed(e.to_string())
        }
    };

    tauri::Builder::default()
        .manage(Arc::new(backend))
        .invoke_handler(tauri::generate_handler![rpc])
        .run(tauri::generate_context!())
        .expect("error while running Artaveo Dental");
}
