//! Regenerate the Recovery Key (Owner only).
//!
//! The Recovery Key is shown once at setup and never stored. If the paper is lost, nothing can
//! show it again, so the Owner may make a new one: the *same* data key is wrapped again with a new
//! Recovery Key. The database itself is not touched.
//!
//! * The live `clinic.rkey` is replaced first, crash-safely (temp file, read back and checked, then
//!   renamed over). At every moment the clinic has one valid `.rkey`.
//! * The `.rkey` files beside existing backups (local folder and the reachable USB / cloud folders)
//!   are re-wrapped too, best effort. Copies that cannot be reached keep the old key.
//! * The old Recovery Key is never needed (it is not stored): the data key comes from the open
//!   database. A backup's `.rkey` is only replaced if its database opens with that same data key,
//!   so a file from another clinic is never overwritten. Running it again is harmless.

use std::fs;
use std::io::Write;
use std::path::Path;

use artaveo_shared::ErrorCode;
use rusqlite::{Connection, OpenFlags};

use crate::backup::list_folder;
use crate::db::KeyFiles;
use crate::error::{CoreError, Result};
use crate::keys::{DataKey, RecoveryKey};

/// What happened to the `.rkey` files of one backup folder.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FolderReport {
    pub updated: u32,
    pub failed: u32,
}

/// Replaces `target` with the data key wrapped by `new`. The new file is written beside the target,
/// read back and unwrapped with `new` (it must give the same data key), and only then renamed over
/// the old one. On any error the old file is left as it was and the temp file is removed.
pub fn replace_checked(target: &Path, new: &RecoveryKey, key: &DataKey) -> Result<()> {
    let tmp = target.with_extension("rkey.tmp");
    let result = (|| -> Result<()> {
        {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(&new.wrap(key))?;
            file.sync_all()?;
        }
        let back = fs::read(&tmp)?;
        if new.unwrap(&back)? != *key {
            return Err(CoreError::api(ErrorCode::Internal, "the new key file did not verify"));
        }
        fs::rename(&tmp, target)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Replaces the clinic's own `.rkey` next to `db_path` with a new Recovery Key and returns that key.
pub fn regenerate_live(db_path: &Path, key: &DataKey) -> Result<RecoveryKey> {
    let new = RecoveryKey::generate();
    replace_checked(&KeyFiles::beside(db_path).recovery, &new, key)?;
    Ok(new)
}

/// True when the encrypted database file opens with `key` (read-only: the file is never changed).
fn opens_with(path: &Path, key: &DataKey) -> bool {
    let Ok(conn) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return false;
    };
    conn.execute_batch(&format!("PRAGMA key = {};", key.sqlcipher_pragma_value())).is_ok()
        && conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).is_ok()
}

/// Re-wraps the `.rkey` beside every backup in `dir` that already has one. A folder that cannot be
/// read gives an empty report.
pub fn rewrap_folder(dir: &Path, key: &DataKey, new: &RecoveryKey) -> FolderReport {
    let mut report = FolderReport::default();
    for (name, _) in list_folder(dir) {
        let db = dir.join(&name);
        let rkey = db.with_extension("rkey");
        if !rkey.exists() {
            continue;
        }
        let done = opens_with(&db, key) && replace_checked(&rkey, new, key).is_ok();
        if done {
            report.updated += 1;
        } else {
            tracing::warn!(file = %name, "backup key file could not be updated to the new Recovery Key");
            report.failed += 1;
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::keys::default_protector;

    /// A clinic database plus a "backup" of it (a VACUUM INTO copy with the key file beside it).
    fn clinic() -> (tempfile::TempDir, std::path::PathBuf, DataKey, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clinic.db");
        let recovery = RecoveryKey::generate();
        let (conn, key) = db::create(&path, default_protector().as_ref(), &recovery).unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY) STRICT; INSERT INTO t VALUES (1);")
            .unwrap();
        drop(conn);
        (dir, path, key, recovery.to_display())
    }

    fn make_backup(dir: &Path, key: &DataKey, name: &str, rkey_from: &Path) {
        let source = db::connect(&dir.join("clinic.db"), key).unwrap();
        db::vacuum_into(&source, &dir.join(name)).unwrap();
        fs::copy(rkey_from, dir.join(name).with_extension("rkey")).unwrap();
    }

    #[test]
    fn live_key_file_gets_a_new_key_and_the_old_one_stops_working() {
        let (_dir, path, key, old) = clinic();
        let new = regenerate_live(&path, &key).unwrap();
        let blob = fs::read(path.with_extension("rkey")).unwrap();
        assert_eq!(new.unwrap(&blob).unwrap(), key, "the new key opens the file and yields the same data key");
        assert!(RecoveryKey::parse(&old).unwrap().unwrap(&blob).is_err(), "the old key no longer works");
        assert_ne!(old, new.to_display());
        assert!(!path.with_extension("rkey.tmp").exists());
        // The database still opens with the unchanged data key.
        let conn = db::connect(&path, &key).unwrap();
        assert_eq!(conn.query_row("SELECT count(*) FROM t", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    }

    #[test]
    fn a_failed_write_leaves_the_old_key_file_intact() {
        let (_dir, path, key, old) = clinic();
        let rkey = path.with_extension("rkey");
        let before = fs::read(&rkey).unwrap();
        // The temp file cannot be created: a folder sits where it would go.
        fs::create_dir(path.with_extension("rkey.tmp")).unwrap();
        assert!(regenerate_live(&path, &key).is_err());
        assert_eq!(fs::read(&rkey).unwrap(), before);
        assert_eq!(RecoveryKey::parse(&old).unwrap().unwrap(&before).unwrap(), key);
    }

    #[test]
    fn backup_key_files_follow_but_foreign_ones_are_left_alone() {
        let (dir, path, key, old) = clinic();
        let backups = dir.path().join("backups");
        fs::create_dir(&backups).unwrap();
        let live_rkey = path.with_extension("rkey");
        let a = "artaveo-20260101-100000-daily.db";
        let b = "artaveo-20260102-100000-daily.db";
        make_backup(&backups, &key, a, &live_rkey);
        make_backup(&backups, &key, b, &live_rkey);
        // A backup of another clinic: same name pattern, different data key.
        let other_path = dir.path().join("other").join("clinic.db");
        let other_recovery = RecoveryKey::generate();
        let (other_conn, _other_key) =
            db::create(&other_path, default_protector().as_ref(), &other_recovery).unwrap();
        other_conn.execute_batch("CREATE TABLE x (id INTEGER PRIMARY KEY) STRICT;").unwrap();
        let foreign = "artaveo-20260103-100000-daily.db";
        db::vacuum_into(&other_conn, &backups.join(foreign)).unwrap();
        drop(other_conn);
        fs::copy(other_path.with_extension("rkey"), backups.join(foreign).with_extension("rkey")).unwrap();
        let foreign_before = fs::read(backups.join(foreign).with_extension("rkey")).unwrap();

        let new = regenerate_live(&path, &key).unwrap();
        let report = rewrap_folder(&backups, &key, &new);
        assert_eq!(report, FolderReport { updated: 2, failed: 1 });

        for name in [a, b] {
            let blob = fs::read(backups.join(name).with_extension("rkey")).unwrap();
            let opened = crate::dump::key_from_recovery(&backups.join(name).with_extension("rkey"), &new.to_display());
            assert_eq!(opened.unwrap(), key, "{name} opens with the new key");
            assert!(RecoveryKey::parse(&old).unwrap().unwrap(&blob).is_err());
        }
        assert_eq!(fs::read(backups.join(foreign).with_extension("rkey")).unwrap(), foreign_before);

        // Running it again is harmless.
        assert_eq!(rewrap_folder(&backups, &key, &new), FolderReport { updated: 2, failed: 1 });
        // A missing folder is an empty report, not an error.
        assert_eq!(rewrap_folder(&dir.path().join("nope"), &key, &new), FolderReport::default());
    }

    #[test]
    fn a_backup_without_a_key_file_is_not_given_one() {
        let (dir, path, key, _old) = clinic();
        let backups = dir.path().join("backups");
        fs::create_dir(&backups).unwrap();
        make_backup(&backups, &key, "artaveo-20260101-100000-daily.db", &path.with_extension("rkey"));
        fs::remove_file(backups.join("artaveo-20260101-100000-daily.rkey")).unwrap();
        let new = regenerate_live(&path, &key).unwrap();
        assert_eq!(rewrap_folder(&backups, &key, &new), FolderReport::default());
        assert!(!backups.join("artaveo-20260101-100000-daily.rkey").exists());
    }
}
