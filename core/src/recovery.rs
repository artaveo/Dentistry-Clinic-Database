//! Crash and corruption recovery (roadmap 11.3).
//!
//! When the clinic database does not open (damaged file, power cut in the wrong moment, a disk
//! problem), the shell starts in Safe Startup Mode. Two ways out:
//!
//! * [`recover_from_latest_backup`] — the key on this computer still works: the newest backup that
//!   passes full validation replaces the damaged file.
//! * The Recovery Key path (`backup.restore_new`): the key is lost or the backup came from another
//!   computer.
//!
//! In both cases the damaged files are never deleted: they move to `<data>/quarantine/<time>/` so
//! that support can still try to read them.

use std::fs;
use std::path::{Path, PathBuf};

use artaveo_shared::{BackupSource, ErrorCode};
use serde_json::json;

use crate::audit::{self, Actor};
use crate::backup;
use crate::clock::now;
use crate::config::Config;
use crate::core::Core;
use crate::db::{self, KeyFiles};
use crate::error::{CoreError, Result};
use crate::keys::KeyProtector;
use crate::restore::inspect;

/// What a successful recovery brought back.
#[derive(Debug, Clone)]
pub struct Recovered {
    pub file_name: String,
    pub created_at: Option<String>,
    pub patients: i64,
    /// Where the damaged files were kept.
    pub quarantined: Option<PathBuf>,
}

fn db_files(db: &Path) -> Vec<PathBuf> {
    vec![
        db.to_path_buf(),
        PathBuf::from(format!("{}-wal", db.display())),
        PathBuf::from(format!("{}-shm", db.display())),
    ]
}

fn stamp() -> String {
    let t = now();
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        t.year(),
        t.month() as u8,
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

/// Moves the database files (and, with `with_keys`, the key files) aside. Nothing is deleted.
/// Returns the folder they went to, or `None` when there was nothing to move.
pub fn quarantine(config: &Config, with_keys: bool) -> Result<Option<PathBuf>> {
    let live = config.db_path();
    let mut files = db_files(&live);
    if with_keys {
        let keys = KeyFiles::beside(&live);
        files.push(keys.local);
        files.push(keys.recovery);
    }
    let present: Vec<PathBuf> = files.into_iter().filter(|f| f.exists()).collect();
    if present.is_empty() {
        return Ok(None);
    }
    let root = config.data_dir.join("quarantine");
    let mut dir = root.join(stamp());
    let mut n = 1;
    while dir.exists() {
        n += 1;
        dir = root.join(format!("{}-{n}", stamp()));
    }
    fs::create_dir_all(&dir)?;
    for f in &present {
        if let Some(name) = f.file_name() {
            fs::rename(f, dir.join(name))?;
        }
    }
    tracing::warn!(folder = %dir.display(), files = present.len(), "damaged clinic files moved to quarantine");
    Ok(Some(dir))
}

/// Puts quarantined files back (a recovery that failed halfway must leave things as they were).
pub fn unquarantine(config: &Config, dir: &Path) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let target = config.data_dir.join(entry.file_name());
        if !target.exists() {
            let _ = fs::rename(entry.path(), target);
        }
    }
    let _ = fs::remove_dir(dir);
}

fn remove_db_files(db: &Path) {
    for f in db_files(db) {
        let _ = fs::remove_file(f);
    }
}

/// Replaces the damaged database with the newest valid local backup. The key on this computer must
/// still open the backups; otherwise the Recovery Key path is the way.
pub fn recover_from_latest_backup(config: &Config, protector: &dyn KeyProtector) -> Result<Recovered> {
    let live = config.db_path();
    let key = db::load_key(&live, protector)?;

    // `candidates_in` lists the newest first; the first one that passes full validation wins.
    let mut chosen = None;
    for c in backup::candidates_in(BackupSource::Local, &config.backup_dir()) {
        let src = Path::new(&c.folder).join(&c.file_name);
        let preview = inspect(&src, &key, &config.data_dir);
        if preview.valid {
            chosen = Some((src, c.file_name, preview));
            break;
        }
    }
    let (src, file_name, preview) =
        chosen.ok_or_else(|| CoreError::api(ErrorCode::BackupInvalid, "no valid backup on this computer"))?;

    let staged = config.data_dir.join("clinic.db.restoring");
    remove_db_files(&staged);
    fs::copy(&src, &staged)?;
    let moved = quarantine(config, false)?;
    let swapped = (|| -> Result<()> {
        fs::rename(&staged, &live)?;
        let conn = db::connect(&live, &key)?;
        db::quick_validate(&conn)?;
        db::full_integrity_check(&conn)?;
        Ok(())
    })();
    if let Err(e) = swapped {
        tracing::error!(error = %e, "recovery from backup failed, putting the files back");
        remove_db_files(&staged);
        remove_db_files(&live);
        if let Some(dir) = &moved {
            unquarantine(config, dir);
        }
        return Err(CoreError::api(ErrorCode::BackupInvalid, e.to_string()));
    }
    tracing::info!(backup = %file_name, patients = preview.patients, "recovered from the newest valid backup");
    Ok(Recovered {
        file_name,
        created_at: preview.created_at,
        patients: preview.patients,
        quarantined: moved,
    })
}

impl Core {
    /// Writes the recovery into the audit log of the reopened database.
    pub fn record_recovery(&self, recovered: &Recovered) {
        let _ = self.with_db(|o| {
            audit::record(
                &o.conn,
                &Actor::system(),
                "db.recovered",
                Some("backup"),
                None,
                None,
                Some(&json!({
                    "backup": recovered.file_name,
                    "quarantine": recovered.quarantined.as_ref().map(|p| p.display().to_string()),
                })),
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use artaveo_shared::methods as m;
    use serde_json::json;

    use super::*;
    use crate::keys::default_protector;
    use crate::tests::T;

    #[test]
    fn a_damaged_database_is_replaced_by_the_newest_valid_backup_and_kept_aside() {
        let t = T::new();
        t.setup();
        let owner = t.login("owner", "owner-pass-1");
        t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
        let config = t.core.config().clone();
        let dir = t._dir;
        drop(t.core);

        // A power cut or a bad disk sector: the file is no longer a database.
        fs::write(config.db_path(), b"this is not a database").unwrap();
        assert!(Core::open(config.clone()).is_err(), "a damaged clinic must not open");

        let recovered = recover_from_latest_backup(&config, default_protector().as_ref()).unwrap();
        assert!(recovered.file_name.starts_with("artaveo-"));
        let kept = recovered.quarantined.clone().expect("the damaged file is kept, not deleted");
        assert_eq!(fs::read(kept.join("clinic.db")).unwrap(), b"this is not a database");

        let core = Core::open(config).unwrap();
        assert!(core.is_set_up());
        core.record_recovery(&recovered);
        drop(dir);
    }

    #[test]
    fn without_a_valid_backup_nothing_is_changed() {
        let t = T::new();
        t.setup();
        let config = t.core.config().clone();
        drop(t.core);
        fs::write(config.db_path(), b"damaged").unwrap();

        let err = recover_from_latest_backup(&config, default_protector().as_ref()).unwrap_err();
        assert_eq!(err.code(), ErrorCode::BackupInvalid);
        assert_eq!(fs::read(config.db_path()).unwrap(), b"damaged", "the file stays where it was");
        assert!(!config.data_dir.join("quarantine").exists());
    }

    #[test]
    fn quarantine_keeps_every_file_and_can_be_undone() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::new(crate::Environment::Test, dir.path().to_path_buf());
        fs::write(config.db_path(), b"db").unwrap();
        fs::write(dir.path().join("clinic.key"), b"key").unwrap();
        let moved = quarantine(&config, true).unwrap().unwrap();
        assert!(!config.db_path().exists());
        assert!(moved.join("clinic.db").exists() && moved.join("clinic.key").exists());
        unquarantine(&config, &moved);
        assert_eq!(fs::read(config.db_path()).unwrap(), b"db");
        assert_eq!(fs::read(dir.path().join("clinic.key")).unwrap(), b"key");
        assert!(quarantine(&Config::new(crate::Environment::Test, dir.path().join("empty")), true)
            .unwrap()
            .is_none());
    }
}
