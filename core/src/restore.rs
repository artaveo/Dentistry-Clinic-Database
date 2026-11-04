//! Restore (roadmap 10.7): `Select Backup -> Validate -> Preview -> safety copy of the current data
//! -> Restore -> Verify`. A backup that does not pass validation can not be restored. Only the Owner
//! may restore, with their password, and it is audited. On a new computer the backup is opened with
//! the Recovery Key (ADR-04).

use std::fs;
use std::path::{Path, PathBuf};

use artaveo_shared::{
    BackupSource, ErrorCode, RestoreCandidate, RestoreNewParams, RestoreParams, RestorePreview,
    RestoreResult, RestoreTarget, ScanBackupsParams, ValidationRule,
};
use rusqlite::Connection;
use serde_json::json;

use crate::audit::{self, Actor};
use crate::auth::{self, OWNER};
use crate::backup;
use crate::core::{Core, Opened};
use crate::db::{self, KeyFiles};
use crate::error::{CoreError, Result};
use crate::keys::{DataKey, RecoveryKey};
use crate::session::Session;

fn remove_db_files(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

fn problem(code: &str, schema: i64) -> RestorePreview {
    RestorePreview {
        valid: false,
        problem: Some(code.into()),
        created_at: None,
        clinic_name: None,
        schema_version: schema,
        patients: 0,
        appointments: 0,
        invoices: 0,
        users: 0,
        last_activity: None,
        current_patients: 0,
        current_last_activity: None,
    }
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0)
}

fn text(conn: &Connection, sql: &str) -> Option<String> {
    conn.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).ok().flatten()
}

/// Opens a *copy* of the backup (the original, maybe on a USB stick, is never touched) and checks
/// it: readable with this key, a clinic database, not newer than this app, no damage.
pub fn inspect(src: &Path, key: &DataKey, work_dir: &Path) -> RestorePreview {
    let tmp = work_dir.join("restore-check.db");
    remove_db_files(&tmp);
    if fs::copy(src, &tmp).is_err() {
        return problem("unreadable", 0);
    }
    let result = (|| {
        let conn = match db::connect(&tmp, key) {
            Ok(c) => c,
            Err(CoreError::WrongKey) => return problem("wrong_key", 0),
            Err(_) => return problem("unreadable", 0),
        };
        if count(&conn, "SELECT COUNT(*) FROM sqlite_master WHERE name = 'db_meta'") == 0 {
            return problem("not_a_backup", 0);
        }
        let schema = count(&conn, "SELECT COALESCE(MAX(version), 0) FROM schema_migrations");
        let known = db::MIGRATIONS.last().map(|m| m.version).unwrap_or(0);
        if schema > known {
            return problem("newer_than_app", schema);
        }
        if db::full_integrity_check(&conn).is_err() {
            return problem("damaged", schema);
        }
        RestorePreview {
            valid: true,
            problem: None,
            created_at: backup::parse_name(
                &src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            )
            .map(crate::clock::iso),
            clinic_name: text(&conn, "SELECT value FROM db_meta WHERE key = 'clinic_name'"),
            schema_version: schema,
            patients: count(&conn, "SELECT COUNT(*) FROM patient WHERE deleted_at IS NULL"),
            appointments: count(&conn, "SELECT COUNT(*) FROM appointment WHERE deleted_at IS NULL"),
            invoices: count(&conn, "SELECT COUNT(*) FROM invoice"),
            users: count(&conn, "SELECT COUNT(*) FROM app_user"),
            last_activity: text(&conn, "SELECT MAX(at) FROM audit_log"),
            current_patients: 0,
            current_last_activity: None,
        }
    })();
    remove_db_files(&tmp);
    result
}

fn find_file(folders: &[PathBuf], name: &str) -> Result<PathBuf> {
    let name = backup::safe_file_name(name)?;
    folders.iter().map(|f| f.join(name)).find(|p| p.is_file()).ok_or_else(|| {
        CoreError::invalid("file_name", ValidationRule::BackupFileUnknown, "backup file not found")
    })
}

fn invalid_backup(p: &RestorePreview) -> CoreError {
    CoreError::api(ErrorCode::BackupInvalid, p.problem.clone().unwrap_or_else(|| "unreadable".into()))
}

impl Core {
    pub(crate) fn restore_candidates(&self, source: BackupSource) -> Result<Vec<RestoreCandidate>> {
        let usb = self.usb();
        self.with_db(|o| {
            let folders = backup::source_folders(&o.conn, &o.key, &self.config, usb.as_ref(), source)?;
            Ok(folders.iter().flat_map(|f| backup::candidates_in(source, f)).collect())
        })
    }

    pub(crate) fn restore_preview(&self, t: &RestoreTarget) -> Result<RestorePreview> {
        let usb = self.usb();
        self.with_db(|o| {
            let folders = backup::source_folders(&o.conn, &o.key, &self.config, usb.as_ref(), t.source)?;
            let src = find_file(&folders, &t.file_name)?;
            let mut preview = inspect(&src, &o.key, &self.config.data_dir);
            preview.current_patients =
                count(&o.conn, "SELECT COUNT(*) FROM patient WHERE deleted_at IS NULL");
            preview.current_last_activity = text(&o.conn, "SELECT MAX(at) FROM audit_log");
            Ok(preview)
        })
    }

    /// Replaces the clinic's data with a backup. Every session ends; the Owner signs in again.
    pub(crate) fn restore_backup(&self, s: &Session, p: &RestoreParams) -> Result<RestoreResult> {
        s.require(auth::perm::BACKUP_RESTORE)?;
        if s.user.role != OWNER {
            return Err(CoreError::forbidden(auth::perm::BACKUP_RESTORE));
        }
        let actor = Actor::user(&s.user.id, &s.user.username);
        let usb = self.usb();
        let staged = self.config.data_dir.join("clinic.db.restoring");
        remove_db_files(&staged);

        // 1. Check everything and stage the chosen file, while the current data is still untouched.
        let (src, safety, preview) = self.with_db(|o| {
            if !auth::check_password(&o.conn, &s.user.id, &p.password)? {
                return Err(CoreError::api(ErrorCode::InvalidCredentials, "wrong password")
                    .on_field("password", ValidationRule::WrongPassword));
            }
            let folders = backup::source_folders(&o.conn, &o.key, &self.config, usb.as_ref(), p.source)?;
            let src = find_file(&folders, &p.file_name)?;
            let preview = inspect(&src, &o.key, &self.config.data_dir);
            if !preview.valid {
                return Err(invalid_backup(&preview));
            }
            // A copy of what is there now, so a wrong restore can be undone (10.2).
            let safety = backup::create(&o.conn, &o.key, &self.config, backup::Kind::PreRestore, &actor)?;
            fs::copy(&src, &staged)?;
            let check = db::connect(&staged, &o.key).and_then(|c| db::full_integrity_check(&c));
            remove_wal(&staged);
            if let Err(e) = check {
                remove_db_files(&staged);
                return Err(CoreError::api(ErrorCode::BackupInvalid, e.to_string()));
            }
            Ok((src, safety, preview))
        })?;

        // 2. Swap the files while nobody can use the database.
        let Some(Opened { conn, key }) = self.lock_db().take() else {
            remove_db_files(&staged);
            return Err(CoreError::api(ErrorCode::NotSetUp, "clinic is not set up"));
        };
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");
        drop(conn);
        let live = self.config.db_path();
        let aside = live.with_extension("replaced");
        let swapped = (|| -> Result<()> {
            remove_db_files(&aside);
            fs::rename(&live, &aside)?;
            remove_wal(&live);
            fs::rename(&staged, &live)?;
            Ok(())
        })();
        let reopened = swapped
            .and_then(|()| db::connect(&live, &key))
            .and_then(|c| db::quick_validate(&c).map(|()| c))
            .and_then(|c| self.install(c, key.clone()));
        if let Err(e) = reopened {
            // Put the old data back and carry on as if nothing happened.
            tracing::error!(error = %e, "restore failed, putting the previous data back");
            remove_db_files(&live);
            let _ = fs::rename(&aside, &live);
            remove_db_files(&staged);
            if let Ok(c) = db::connect(&live, &key) {
                let _ = self.install(c, key.clone());
            }
            return Err(CoreError::api(ErrorCode::BackupInvalid, e.to_string()));
        }
        remove_db_files(&aside);
        self.lock_sessions().clear();

        // 3. The images that only the backup medium has come back too; the register is brought up to date.
        if let Some(att) = src.parent().map(|d| d.join("attachments")) {
            let _ = backup::sync_attachments(&att, &self.config.attachments_dir());
        }
        self.with_db(|o| {
            backup::reconcile(&o.conn, &self.config.backup_dir())?;
            audit::record(
                &o.conn,
                &actor,
                "backup.restore",
                Some("backup"),
                None,
                None,
                Some(&json!({
                    "source": p.source,
                    "file": p.file_name,
                    "safety_backup": safety.file_name,
                    "patients": preview.patients,
                })),
            )
        })?;
        self.start_integrity_check();
        Ok(RestoreResult {
            safety_backup: safety.file_name,
            restored: p.file_name.clone(),
            patients: preview.patients,
        })
    }

    // ───────────────────────────── a new computer (no clinic yet) ─────────────────────────────

    /// Backups on connected USB drives (their `ArtaveoBackup` folder) and in one folder the user chose.
    pub(crate) fn scan_backups(&self, p: &ScanBackupsParams) -> Result<Vec<RestoreCandidate>> {
        if self.is_set_up() {
            return Err(CoreError::api(ErrorCode::AlreadySetUp, "this computer already has a clinic"));
        }
        let mut out = Vec::new();
        for v in self.usb().volumes() {
            out.extend(backup::candidates_in(BackupSource::Usb, &crate::usb::backup_folder(&v.mount)));
        }
        if let Some(folder) = p.folder.as_deref().map(str::trim).filter(|f| !f.is_empty()) {
            let dir = Path::new(folder);
            // The cloud copy sits in an `ArtaveoDental` sub-folder of the synced folder.
            for d in [dir.to_path_buf(), backup::cloud_dir(folder)] {
                out.extend(backup::candidates_in(BackupSource::Cloud, &d));
            }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }

    /// Restores a backup on a computer that has no clinic yet, using the Recovery Key printed at
    /// setup of the old computer. The data key found in the backup's `.rkey` is protected for *this*
    /// computer (DPAPI) so the clinic opens normally from now on.
    pub(crate) fn restore_new_computer(&self, p: &RestoreNewParams) -> Result<()> {
        if self.is_set_up() {
            return Err(CoreError::api(ErrorCode::AlreadySetUp, "this computer already has a clinic"));
        }
        let folder = PathBuf::from(p.folder.trim());
        let src = find_file(std::slice::from_ref(&folder), &p.file_name)?;
        let wrapped = fs::read(src.with_extension("rkey")).map_err(|_| {
            CoreError::invalid(
                "file_name",
                ValidationRule::BackupFileUnknown,
                "the key file (.rkey) is missing",
            )
        })?;
        let key = RecoveryKey::parse(&p.recovery_key)
            .ok()
            .and_then(|rk| rk.unwrap(&wrapped).ok())
            .ok_or_else(|| {
                CoreError::api(ErrorCode::RecoveryKeyInvalid, "recovery key does not match this backup")
                    .on_field("recovery_key", ValidationRule::RecoveryKey)
            })?;

        fs::create_dir_all(&self.config.data_dir)?;
        let staged = self.config.data_dir.join("clinic.db.restoring");
        remove_db_files(&staged);
        let preview = inspect(&src, &key, &self.config.data_dir);
        if !preview.valid {
            return Err(invalid_backup(&preview));
        }
        fs::copy(&src, &staged)?;

        let live = self.config.db_path();
        let files = KeyFiles::beside(&live);
        let result = (|| -> Result<()> {
            remove_db_files(&live);
            fs::write(&files.local, self.protector.protect(&key)?)?;
            fs::write(&files.recovery, &wrapped)?;
            fs::rename(&staged, &live)?;
            let conn = db::connect(&live, &key)?;
            db::quick_validate(&conn)?;
            self.install(conn, key.clone())
        })();
        if let Err(e) = result {
            remove_db_files(&staged);
            remove_db_files(&live);
            let _ = fs::remove_file(&files.local);
            let _ = fs::remove_file(&files.recovery);
            return Err(CoreError::api(ErrorCode::BackupInvalid, e.to_string()));
        }
        if let Some(att) = src.parent().map(|d| d.join("attachments")) {
            let _ = backup::sync_attachments(&att, &self.config.attachments_dir());
        }
        self.with_db(|o| {
            backup::reconcile(&o.conn, &self.config.backup_dir())?;
            audit::record(
                &o.conn,
                &Actor::system(),
                "backup.restore_new_computer",
                Some("backup"),
                None,
                None,
                Some(&json!({"file": p.file_name, "patients": preview.patients})),
            )
        })?;
        self.start_integrity_check();
        Ok(())
    }
}

/// The write-ahead files that a connection leaves beside a database file.
fn remove_wal(path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_a_backup_is_reported_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("artaveo-20261104-190000-daily.db");
        fs::write(&file, b"this is not a database at all, just some text").unwrap();
        let p = inspect(&file, &DataKey::generate(), dir.path());
        assert!(!p.valid);
        assert_eq!(p.problem.as_deref(), Some("wrong_key"));
        let p = inspect(&dir.path().join("missing.db"), &DataKey::generate(), dir.path());
        assert_eq!(p.problem.as_deref(), Some("unreadable"));
        assert!(!dir.path().join("restore-check.db").exists(), "the work copy is removed");
    }
}
