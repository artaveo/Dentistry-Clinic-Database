//! Reads a clinic backup from outside the program (support, the owner's own scripts).
//!
//!   artaveo-dump --backup artaveo-20261110-190000-daily.db --out dump.sql
//!   artaveo-dump --backup artaveo-....db --sqlite plain.sqlite
//!
//! The Recovery Key comes from the environment variable ARTAVEO_RECOVERY_KEY (so it does not stay in
//! the shell history) or from `--recovery-key`. The `.rkey` file beside the backup is used. Use a
//! backup file, not the live `clinic.db`. The output holds every patient's data: keep it safe.
//! Attachments (X-rays, documents) are separate files in `attachments/` and are not part of it.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use artaveo_core::dump;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn fail(message: &str) -> ! {
    eprintln!("artaveo-dump: {message}");
    std::process::exit(1);
}

fn main() {
    let Some(backup) = arg("--backup").map(PathBuf::from) else {
        fail("usage: artaveo-dump --backup <backup.db> [--out dump.sql] [--sqlite plain.sqlite] [--force]");
    };
    let recovery = arg("--recovery-key")
        .or_else(|| std::env::var("ARTAVEO_RECOVERY_KEY").ok())
        .unwrap_or_else(|| fail("the Recovery Key is missing: set ARTAVEO_RECOVERY_KEY or pass --recovery-key"));
    let force = std::env::args().any(|a| a == "--force");
    let sqlite_out = arg("--sqlite").map(PathBuf::from);
    let sql_out = match (arg("--out"), &sqlite_out) {
        (Some(p), _) => Some(PathBuf::from(p)),
        (None, Some(_)) => None,
        (None, None) => Some(backup.with_extension("sql")),
    };
    if backup.file_name().is_some_and(|n| n == "clinic.db") {
        fail("this is the live database; use a backup file from the backups folder instead");
    }
    for out in sql_out.iter().chain(sqlite_out.iter()) {
        if out.exists() && !force {
            fail(&format!("{} already exists (use --force to replace it)", out.display()));
        }
    }

    let key = dump::key_from_recovery(&backup.with_extension("rkey"), &recovery)
        .unwrap_or_else(|e| fail(&format!("the backup could not be opened with this Recovery Key: {e}")));
    let work = std::env::temp_dir().join(format!("artaveo-dump-{}", std::process::id()));
    let result = (|| -> artaveo_core::error::Result<()> {
        let conn = dump::open_copy(&backup, &key, &work)?;
        if let Some(path) = &sql_out {
            let mut file = BufWriter::new(File::create(path)?);
            let report = dump::write_sql(&conn, &mut file)?;
            println!("{}: {} tables, {} rows", path.display(), report.tables, report.rows);
            for s in &report.skipped {
                println!("  not included (rebuilt by the program): {s}");
            }
        }
        if let Some(path) = &sqlite_out {
            if path.exists() {
                std::fs::remove_file(path)?;
            }
            dump::export_plain_sqlite(&conn, path)?;
            println!("{}: unencrypted SQLite file", path.display());
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&work);
    if let Err(e) = result {
        fail(&e.to_string());
    }
    eprintln!("WARNING: the output has patient data and is not encrypted. Keep it safe and delete it after use.");
}
