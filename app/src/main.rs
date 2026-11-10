// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Tauri shell (ADR-01/02): owns the window and forwards every UI request to the Rust Core through
//! one IPC command. It contains no business logic. In LAN mode (Phase 7) it also hosts the clinic
//! server on the server computer, or forwards the requests to the server on a client computer.

use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use artaveo_core::{Config, Core};
use artaveo_lan::client::{self, ClientError, ClientStore, LanClient, Notice};
use artaveo_lan::server::ServerOptions;
use serde_json::{json, Value};
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::broadcast::error::RecvError;

mod autostart;
mod print;
use artaveo_shared::{ErrorCode, RpcError, RpcRequest, RpcResponse};

/// The folder where the Core writes Excel exports (OF-022).
struct ExportsDir(std::path::PathBuf);

/// Opens Explorer with an exported file selected. Only files directly inside the exports
/// folder (Excel exports) or its documents folder (PDFs of printed documents) are accepted, so the
/// window cannot make Explorer open an arbitrary path.
#[tauri::command]
fn reveal_export(
    exports: tauri::State<'_, ExportsDir>,
    documents: tauri::State<'_, print::DocumentsDir>,
    path: String,
) -> Result<(), String> {
    let file = std::path::PathBuf::from(&path);
    // Documents may sit in year/month folders below the documents folder; `..` is never accepted.
    let plain = !file.components().any(|c| matches!(c, std::path::Component::ParentDir));
    let inside = plain
        && file
            .parent()
            .is_some_and(|dir| dir == exports.0.as_path() || dir.starts_with(documents.0.as_path()));
    if !inside || !file.is_file() {
        return Err("only exported files can be shown from here".into());
    }
    std::process::Command::new("explorer")
        .arg(format!("/select,{}", file.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// What answers the UI's requests: this computer's own Core (a single computer or the clinic
/// server), or the clinic server over the network (roadmap Phase 7), or why neither could start
/// (e.g. the database key is missing on this computer and the clinic must be restored with its
/// Recovery Key).
enum Backend {
    Ready(Arc<Core>),
    Client(Arc<LanClient>),
    /// Safe Startup Mode (roadmap 11.3): the clinic did not open.
    Failed(Problem),
    /// Safe Startup Mode with a core that has not loaded the damaged database, so that a backup can be
    /// restored with the Recovery Key.
    Recovery(Arc<Core>, Problem),
}

/// Why the clinic did not open: a short code the window translates, and the technical detail for the log.
#[derive(Clone)]
struct Problem {
    reason: &'static str,
    detail: String,
}

fn problem_of(e: &artaveo_core::CoreError) -> Problem {
    use artaveo_core::CoreError as E;
    let reason = match e {
        E::Key(_) => "key_unavailable",
        E::WrongKey => "wrong_key",
        E::Integrity(_) | E::Sql(_) => "damaged",
        E::DatabaseNewerThanApp { .. } => "newer_than_app",
        E::MigrationDrift { .. } | E::MigrationFailed { .. } => "migration",
        E::Io(io) if io.kind() == std::io::ErrorKind::NotFound => "key_unavailable",
        _ => "other",
    };
    Problem { reason, detail: e.to_string() }
}

/// Opens this computer's own Core and starts its background work (daily backup, license check).
fn open_core(config: &Config) -> Backend {
    match Core::open(config.clone()) {
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
                // The 7-day license check (roadmap 10.8); quiet when offline.
                tick.run_license_check();
                std::thread::sleep(Duration::from_secs(300));
            });
            Backend::Ready(core)
        }
        Err(e) => {
            let p = problem_of(&e);
            tracing::error!(error = %e, reason = p.reason, "core failed to start");
            Backend::Failed(p)
        }
    }
}

/// The running application. The backend can be swapped when this computer is paired with a server
/// or unpaired from it, without restarting.
struct Shell {
    backend: RwLock<Arc<Backend>>,
    config: Config,
    tasks: Mutex<Vec<tauri::async_runtime::JoinHandle<()>>>,
}

impl Shell {
    fn current(&self) -> Arc<Backend> {
        self.backend.read().unwrap().clone()
    }

    fn replace(&self, backend: Backend, tasks: Vec<tauri::async_runtime::JoinHandle<()>>) {
        let mut running = self.tasks.lock().unwrap();
        for t in running.drain(..) {
            t.abort();
        }
        *self.backend.write().unwrap() = Arc::new(backend);
        running.extend(tasks);
    }
}

#[tauri::command]
async fn rpc(state: tauri::State<'_, Arc<Shell>>, request: RpcRequest) -> Result<RpcResponse, String> {
    let backend = state.current();
    match backend.as_ref() {
        Backend::Client(c) => Ok(c.call(request).await),
        Backend::Ready(core) | Backend::Recovery(core, _) => {
            let core = core.clone();
            // Argon2id and backups take real CPU time: keep them off the UI thread.
            tauri::async_runtime::spawn_blocking(move || core.handle(request))
                .await
                .map_err(|e| e.to_string())
        }
        Backend::Failed(p) => Ok(RpcResponse::Error {
            error: RpcError {
                code: ErrorCode::Internal,
                detail: format!("startup failed: {}", p.detail),
                field: None,
                rule: None,
            },
        }),
    }
}

/// Starts a client connection and the feed of its live events and connection state to the window.
fn start_client(app: tauri::AppHandle, client: Arc<LanClient>) -> Vec<tauri::async_runtime::JoinHandle<()>> {
    let mut rx = client.subscribe();
    let run = tauri::async_runtime::spawn(client.run());
    let feed = tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(Notice::Live(e)) => {
                    let _ = app.emit("artaveo://live", e);
                }
                Ok(Notice::Online) => {
                    let _ = app.emit("artaveo://connection", json!({"online": true, "revoked": false}));
                }
                Ok(Notice::Offline) => {
                    let _ = app.emit("artaveo://connection", json!({"online": false, "revoked": false}));
                }
                Ok(Notice::Revoked) => {
                    let _ = app.emit("artaveo://connection", json!({"online": false, "revoked": true}));
                }
                Ok(Notice::Incompatible(server_version)) => {
                    let _ = app.emit(
                        "artaveo://connection",
                        json!({"online": false, "revoked": false, "incompatible": server_version}),
                    );
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            }
        }
    });
    vec![run, feed]
}

/// Server side: every change made on this computer or by a client reaches this window too.
fn forward_server_events(app: tauri::AppHandle, core: Arc<Core>) -> tauri::async_runtime::JoinHandle<()> {
    let mut rx = core.lan().subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(e) => {
                    let _ = app.emit("artaveo://live", e);
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            }
        }
    })
}

/// How this computer takes part in the clinic's network, for the first screens.
#[tauri::command]
fn app_mode(state: tauri::State<'_, Arc<Shell>>) -> Value {
    match state.current().as_ref() {
        Backend::Client(c) => {
            json!({"mode": "client", "server_name": c.server_name(), "online": c.is_online()})
        }
        _ => json!({"mode": "local"}),
    }
}

/// Clinic servers announcing themselves on the network (roadmap 7.2: no typing of addresses).
#[tauri::command]
async fn lan_discover() -> Vec<Value> {
    client::discover(Duration::from_secs(3))
        .await
        .into_iter()
        .map(|f| json!({"clinic_name": f.clinic_name, "addr": f.addr, "fp_hint": f.fp_hint}))
        .collect()
}

/// Pairs this computer with a server and turns it into a client. The answer says what went wrong
/// (`wrong_code`, `closed`, `limit_reached`, `unreachable`, `not_authentic`) so the screen can say it.
#[tauri::command]
async fn lan_pair(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Shell>>,
    addr: String,
    code: String,
    device_name: String,
) -> Result<Value, String> {
    let shell = state.inner().clone();
    if matches!(shell.current().as_ref(), Backend::Ready(core) if core.is_set_up()) {
        return Ok(json!({"ok": false, "reason": "already_set_up"}));
    }
    let addr = client::normalize_addr(&addr);
    match client::pair(&addr, code.trim(), device_name.trim()).await {
        Ok(paired) => {
            let store = ClientStore::new(&shell.config.data_dir);
            store.save(&paired).map_err(|e| e.to_string())?;
            let name = paired.server_name.clone();
            let c = LanClient::new(paired, Some(store));
            let tasks = start_client(app, c.clone());
            shell.replace(Backend::Client(c), tasks);
            Ok(json!({"ok": true, "server_name": name}))
        }
        Err(e) => Ok(match e {
            ClientError::WrongCode { attempts_left } => {
                json!({"ok": false, "reason": "wrong_code", "attempts_left": attempts_left})
            }
            ClientError::Closed => json!({"ok": false, "reason": "closed"}),
            ClientError::LimitReached => json!({"ok": false, "reason": "limit_reached"}),
            ClientError::ServerNotAuthentic => json!({"ok": false, "reason": "not_authentic"}),
            ClientError::NotPaired | ClientError::Incompatible(_) | ClientError::Unreachable(_) => {
                json!({"ok": false, "reason": "unreachable"})
            }
        }),
    }
}

/// Forgets the server: this computer is a fresh installation again.
#[tauri::command]
fn lan_unpair(state: tauri::State<'_, Arc<Shell>>) -> Result<(), String> {
    let shell = state.inner().clone();
    if !matches!(shell.current().as_ref(), Backend::Client(_)) {
        return Err("this computer is not a client".into());
    }
    ClientStore::new(&shell.config.data_dir).clear();
    let backend = match Core::open(shell.config.clone()) {
        Ok(core) => Backend::Ready(Arc::new(core)),
        Err(e) => Backend::Failed(problem_of(&e)),
    };
    shell.replace(backend, Vec::new());
    Ok(())
}

#[tauri::command]
fn autostart_get() -> bool {
    autostart::is_enabled()
}

#[tauri::command]
fn autostart_set(on: bool) -> Result<(), String> {
    autostart::set(on)
}

/// Brings Artaveo to the front when it runs in the background (tray icon, second start).
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// The clinic server keeps running when its window is closed, so the other computers stay connected.
fn keeps_running(app: &tauri::AppHandle) -> bool {
    let shell = app.state::<Arc<Shell>>();
    let backend = shell.current();
    matches!(backend.as_ref(), Backend::Ready(core) if core.lan_enabled())
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let open = MenuItem::with_id(app, "open", "Open Artaveo Dental / باز کردن", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit (stops the server) / خروج", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("Artaveo Dental")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// The background work a running backend needs: the LAN server on a clinic server, the connection
/// feed on a client.
fn backend_tasks(handle: &tauri::AppHandle, backend: &Backend) -> Vec<tauri::async_runtime::JoinHandle<()>> {
    let mut tasks = Vec::new();
    match backend {
        Backend::Ready(core) => {
            // The clinic server: listens while "this computer is the clinic server" is on.
            tasks.push(tauri::async_runtime::spawn(artaveo_lan::service::supervise(
                core.clone(),
                ServerOptions::default(),
            )));
            tasks.push(forward_server_events(handle.clone(), core.clone()));
        }
        Backend::Client(c) => tasks.extend(start_client(handle.clone(), c.clone())),
        Backend::Failed(_) | Backend::Recovery(..) => {}
    }
    tasks
}

/// Roadmap 11.3 and the support goal: a crash must say where it happened. The panic message, the
/// exact file and line, the thread and a backtrace go to the log and to `crashes/crash-<time>.txt`,
/// which the support report lists. (Patient data is never in a panic message: the Core logs ids, not
/// names.)
fn install_panic_hook(config: &Config) {
    let dir = config.data_dir.join("crashes");
    std::panic::set_hook(Box::new(move |info| {
        let place = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_default();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(%place, %message, %thread, "PANIC");
        let _ = std::fs::create_dir_all(&dir);
        let stamp = artaveo_core::clock::now_iso().replace([':', '.'], "-");
        let text = format!(
            "Artaveo Dental crash\nversion: {} ({})\narch: {}\nthread: {thread}\nwhere: {place}\nmessage: {message}\n\n{backtrace}\n",
            artaveo_core::APP_VERSION,
            artaveo_core::GIT_COMMIT,
            artaveo_core::sysinfo::build_arch(),
        );
        let _ = std::fs::write(dir.join(format!("crash-{stamp}.txt")), text);
    }));
}

/// What the window needs to know at start: did the clinic open, and if not, why.
fn startup_json(shell: &Shell) -> Value {
    let backend = shell.current();
    match backend.as_ref() {
        Backend::Failed(p) | Backend::Recovery(_, p) => {
            let recovering = matches!(backend.as_ref(), Backend::Recovery(..));
            // The key on this computer still works, so the newest valid backup can be used at once.
            let can_recover = matches!(p.reason, "damaged" | "migration" | "other");
            json!({"safe": true, "reason": p.reason, "detail": p.detail, "recovering": recovering, "can_recover": can_recover})
        }
        _ => json!({"safe": false}),
    }
}

#[tauri::command]
fn startup_state(state: tauri::State<'_, Arc<Shell>>) -> Value {
    startup_json(&state)
}

/// Safe Startup Mode: try to open the clinic again.
#[tauri::command]
fn safe_retry(app: tauri::AppHandle, state: tauri::State<'_, Arc<Shell>>) -> Value {
    let shell = state.inner().clone();
    let backend = open_core(&shell.config);
    let tasks = backend_tasks(&app, &backend);
    shell.replace(backend, tasks);
    startup_json(&shell)
}

/// Safe Startup Mode: replace the damaged database with the newest valid local backup. The damaged
/// files are kept in the `quarantine` folder. The answer says which backup came back.
#[tauri::command]
async fn safe_recover(app: tauri::AppHandle, state: tauri::State<'_, Arc<Shell>>) -> Result<Value, String> {
    let shell = state.inner().clone();
    if !matches!(shell.current().as_ref(), Backend::Failed(_)) {
        return Err("not_in_safe_mode".into());
    }
    let config = shell.config.clone();
    let done = tauri::async_runtime::spawn_blocking(move || {
        artaveo_core::recovery::recover_from_latest_backup(&config, artaveo_core::keys::default_protector().as_ref())
    })
    .await
    .map_err(|e| e.to_string())?;
    let recovered = match done {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "recovery from the newest backup failed");
            return Err(if e.code() == ErrorCode::BackupInvalid { "no_valid_backup".into() } else { e.to_string() });
        }
    };
    let backend = open_core(&shell.config);
    if let Backend::Ready(core) = &backend {
        core.record_recovery(&recovered);
    }
    let tasks = backend_tasks(&app, &backend);
    let opened = matches!(backend, Backend::Ready(_));
    shell.replace(backend, tasks);
    Ok(json!({
        "ok": opened,
        "file_name": recovered.file_name,
        "created_at": recovered.created_at,
        "patients": recovered.patients,
    }))
}

/// Safe Startup Mode: switch to a core that lets the owner restore a backup with the Recovery Key.
#[tauri::command]
fn safe_enter_restore(state: tauri::State<'_, Arc<Shell>>) -> Result<(), String> {
    let shell = state.inner().clone();
    let problem = match shell.current().as_ref() {
        Backend::Failed(p) => p.clone(),
        Backend::Recovery(..) => return Ok(()),
        _ => return Err("not_in_safe_mode".into()),
    };
    let core = Core::open_for_recovery(shell.config.clone()).map_err(|e| e.to_string())?;
    shell.replace(Backend::Recovery(Arc::new(core), problem), Vec::new());
    Ok(())
}

/// Support report that works even when the clinic does not open (roadmap 11.4).
#[tauri::command]
fn safe_report(state: tauri::State<'_, Arc<Shell>>) -> Result<Value, String> {
    let shell = state.inner().clone();
    let mut facts = artaveo_core::support::base_facts(&shell.config);
    if let Backend::Failed(p) | Backend::Recovery(_, p) = shell.current().as_ref() {
        facts.push(("startup_problem".into(), p.reason.into()));
        facts.push(("startup_detail".into(), p.detail.clone()));
    }
    let report = artaveo_core::support::write_report(&shell.config, &facts).map_err(|e| e.to_string())?;
    Ok(json!({"file_name": report.file_name, "path": report.path.display().to_string()}))
}

/// Errors the window itself hit (a screen that crashed, a failed promise) go to the same log as the
/// Core's, with the screen they came from, so that one search finds the whole story.
#[tauri::command]
fn log_ui_error(place: String, message: String, stack: Option<String>) {
    let cut = |s: &str, n: usize| s.chars().take(n).collect::<String>();
    tracing::error!(
        target: "ui",
        place = %cut(&place, 200),
        message = %cut(&message, 600),
        stack = %cut(&stack.unwrap_or_default(), 2000),
        "UI ERROR"
    );
}

/// An update found by `update_check`, waiting for the user to confirm.
struct PendingUpdate(Mutex<Option<tauri_plugin_updater::Update>>);

/// Looks for a newer signed version (roadmap 11.2). Quiet when offline: the window shows nothing.
#[tauri::command]
async fn update_check(app: tauri::AppHandle, pending: tauri::State<'_, PendingUpdate>) -> Result<Value, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            let info = json!({"available": true, "version": update.version, "current": update.current_version, "notes": update.body});
            *pending.0.lock().unwrap() = Some(update);
            Ok(info)
        }
        Ok(None) => Ok(json!({"available": false})),
        Err(e) => {
            tracing::debug!(error = %e, "update check skipped");
            Err(e.to_string())
        }
    }
}

/// Downloads, verifies the signature of and installs the update found before. The window has already
/// made the safety backup (`update.prepare`). The installer closes the program and starts it again.
#[tauri::command]
async fn update_install(app: tauri::AppHandle, pending: tauri::State<'_, PendingUpdate>) -> Result<(), String> {
    let update = pending.0.lock().unwrap().take().ok_or("no update was found")?;
    tracing::info!(version = %update.version, "installing update");
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| {
        tracing::error!(error = %e, "update install failed");
        e.to_string()
    })?;
    app.restart()
}

fn main() {
    let config = Config::from_env();
    let _log_guard = artaveo_core::logging::init(&config.log_dir(), config.environment).ok();
    install_panic_hook(&config);
    let exports_dir = ExportsDir(config.exports_dir());
    let documents_dir = print::DocumentsDir(config.exports_dir().join("documents"));
    tracing::info!(
        version = artaveo_core::APP_VERSION,
        commit = artaveo_core::GIT_COMMIT,
        arch = artaveo_core::sysinfo::build_arch(),
        env = config.environment.code(),
        data_dir = %config.data_dir.display(),
        "starting Artaveo Dental"
    );
    // Started by Windows at sign-in (see the Network page): stay out of the way until opened.
    let background = std::env::args().any(|a| a == "--background");

    // A computer paired with a clinic server has no database of its own.
    let store = ClientStore::new(&config.data_dir);
    let client =
        store.exists().then(|| store.load()).flatten().map(|p| LanClient::new(p, Some(store.clone())));

    let backend = match &client {
        Some(c) => {
            tracing::info!(server = %c.server_name(), "client mode");
            Backend::Client(c.clone())
        }
        None => open_core(&config),
    };
    let shell =
        Arc::new(Shell { backend: RwLock::new(Arc::new(backend)), config, tasks: Mutex::new(Vec::new()) });

    tauri::Builder::default()
        // A second start only brings the running one (and its server) to the front.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .manage(shell.clone())
        .manage(exports_dir)
        .manage(documents_dir)
        .manage(PendingUpdate(Mutex::new(None)))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            rpc,
            reveal_export,
            app_mode,
            lan_discover,
            lan_pair,
            lan_unpair,
            autostart_get,
            autostart_set,
            print::list_printers,
            print::printer_papers,
            print::print_page,
            print::save_pdf,
            startup_state,
            safe_retry,
            safe_recover,
            safe_enter_restore,
            safe_report,
            log_ui_error,
            update_check,
            update_install
        ])
        .setup(move |app| {
            let tasks = backend_tasks(app.handle(), shell.current().as_ref());
            shell.tasks.lock().unwrap().extend(tasks);
            build_tray(app)?;
            if background {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if keeps_running(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Artaveo Dental");
}
