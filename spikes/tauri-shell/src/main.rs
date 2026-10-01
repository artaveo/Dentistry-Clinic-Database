// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod print;

use serde::Serialize;

#[derive(Serialize)]
struct DbReport {
    sqlcipher: String,
    protector: String,
    path: String,
    recovery_key: String,
    reopened: bool,
}

/// Spike 1: Tauri + Rust Core + SQLCipher + DPAPI, end to end on the target OS.
#[cfg(feature = "db")]
#[tauri::command]
fn db_selftest(app: tauri::AppHandle) -> Result<DbReport, String> {
    use key_protect::{default_protector, RecoveryKey};
    use sqlcipher_core::Database;
    use tauri::Manager;

    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("spike.db");
    for ext in ["db", "db-wal", "db-shm", "key", "rkey"] {
        let _ = std::fs::remove_file(path.with_extension(ext));
    }
    let protector = default_protector();
    let rk = RecoveryKey::generate();
    let db = Database::create(&path, protector.as_ref(), &rk).map_err(|e| e.to_string())?;
    let sqlcipher = db.sqlcipher_version().map_err(|e| e.to_string())?;
    drop(db);
    let reopened = Database::open(&path, protector.as_ref()).is_ok();
    Ok(DbReport {
        sqlcipher,
        protector: format!("{:?}", protector.kind()),
        path: path.display().to_string(),
        recovery_key: rk.to_display(),
        reopened,
    })
}

#[cfg(not(feature = "db"))]
#[tauri::command]
fn db_selftest(_app: tauri::AppHandle) -> Result<DbReport, String> {
    Err("built without the `db` feature".into())
}

#[derive(Serialize)]
struct Today {
    shamsi: String,
    month_dari: &'static str,
    month_pashto: &'static str,
}

#[tauri::command]
fn shamsi_date(year: i32, month: u8, day: u8) -> Result<Today, String> {
    let s = shamsi::GregorianDate { year, month, day }.to_shamsi().map_err(|e| format!("{e:?}"))?;
    Ok(Today {
        shamsi: shamsi::digits::to_persian(&s.to_iso_like()),
        month_dari: shamsi::MONTHS_DARI[s.month as usize - 1],
        month_pashto: shamsi::MONTHS_PASHTO[s.month as usize - 1],
    })
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            db_selftest,
            shamsi_date,
            print::list_printers,
            print::printer_capabilities,
            print::print_escpos_png,
            print::silent_print,
            print::save_pdf,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
