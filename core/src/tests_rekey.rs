//! Regenerate the Recovery Key through the public RPC entry point (Owner only).

use std::fs;
use std::sync::Arc;

use artaveo_shared::{methods as m, ErrorCode};
use serde_json::json;

use crate::tests::T;
use crate::usb::{FakeUsb, UsbVolume};

fn rkey_opens(t: &T, key: &str) -> bool {
    let file = t.core.config().db_path().with_extension("rkey");
    crate::dump::key_from_recovery(&file, key).is_ok()
}

#[test]
fn the_owner_gets_a_new_key_and_the_old_paper_stops_working() {
    let t = T::new();
    let old = t.setup();
    let tok = t.login("owner", "owner-pass-1");
    let b = t.ok(m::BACKUP_CREATE, json!({}), Some(&tok));
    let name = b["file_name"].as_str().unwrap().to_string();
    let backup_rkey = t.core.config().backup_dir().join(&name).with_extension("rkey");
    assert!(crate::dump::key_from_recovery(&backup_rkey, &old).is_ok());

    let r = t.ok(m::BACKUP_REGENERATE_KEY, json!({"password": "owner-pass-1"}), Some(&tok));
    let new = r["recovery_key"].as_str().unwrap().to_string();
    assert_eq!(new.len(), 41, "same printable form as setup: {new}");
    assert_ne!(new, old);
    assert_eq!(
        (r["local_updated"].as_u64(), r["usb_updated"].as_u64(), r["failed"].as_u64()),
        (Some(1), Some(0), Some(0))
    );

    // The live key file and the local backup's key file: new key opens, old key does not.
    assert!(rkey_opens(&t, &new) && !rkey_opens(&t, &old));
    assert!(crate::dump::key_from_recovery(&backup_rkey, &new).is_ok());
    assert!(crate::dump::key_from_recovery(&backup_rkey, &old).is_err());
    // The data key did not change: the clinic keeps working and a new backup is made normally.
    t.ok(m::SYSTEM_INFO, json!({}), Some(&tok));
    t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad"}), Some(&tok));

    // The audit log says who and when, never the key.
    let log = t.ok(m::AUDIT_LIST, json!({"limit": 500, "offset": 0}), Some(&tok)).to_string();
    assert!(t.audit_actions(&tok).contains(&"backup.regenerate_key".to_string()));
    assert!(!log.contains(&new) && !log.contains(&old));

    // Owner password recovery: the old paper is refused, the new one works.
    assert_eq!(
        t.call(m::AUTH_RECOVER_OWNER, json!({"recovery_key": old, "new_password": "brand-new-pass"}), None),
        Err(ErrorCode::RecoveryKeyInvalid)
    );
    t.ok(m::AUTH_RECOVER_OWNER, json!({"recovery_key": new, "new_password": "brand-new-pass"}), None);
    t.login("owner", "brand-new-pass");
}

#[test]
fn a_backup_made_before_regenerating_restores_on_a_new_computer_with_the_new_key_only() {
    let old_pc = T::new();
    let old = old_pc.setup();
    let tok = old_pc.login("owner", "owner-pass-1");
    old_pc.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad"}), Some(&tok));
    let b = old_pc.ok(m::BACKUP_CREATE, json!({}), Some(&tok));
    let name = b["file_name"].as_str().unwrap().to_string();
    let new = old_pc.ok(m::BACKUP_REGENERATE_KEY, json!({"password": "owner-pass-1"}), Some(&tok))
        ["recovery_key"]
        .as_str()
        .unwrap()
        .to_string();

    let folder = tempfile::tempdir().unwrap();
    let from = old_pc.core.config().backup_dir().join(&name);
    fs::copy(&from, folder.path().join(&name)).unwrap();
    fs::copy(from.with_extension("rkey"), folder.path().join(&name).with_extension("rkey")).unwrap();
    let folder = folder.path().display().to_string();

    let pc = T::new();
    let with_old = json!({"folder": folder, "file_name": name, "recovery_key": old});
    assert_eq!(pc.call(m::BACKUP_RESTORE_NEW, with_old, None), Err(ErrorCode::RecoveryKeyInvalid));
    assert!(!pc.core.is_set_up());
    pc.ok(m::BACKUP_RESTORE_NEW, json!({"folder": folder, "file_name": name, "recovery_key": new}), None);
    let tok = pc.login("owner", "owner-pass-1");
    let patients = pc.ok(m::PATIENTS_LIST, json!({"limit": 50, "offset": 0}), Some(&tok));
    assert_eq!(patients["items"].as_array().unwrap().len(), 1);
}

#[test]
fn a_trusted_usb_copy_follows_and_the_result_counts_it() {
    let t = T::new();
    t.setup();
    let tok = t.login("owner", "owner-pass-1");
    let stick = tempfile::tempdir().unwrap();
    let usb = Arc::new(FakeUsb::default());
    t.core.set_usb(usb.clone());
    usb.0.lock().unwrap().push(UsbVolume {
        mount: stick.path().to_path_buf(),
        vendor: "SanDisk".into(),
        product: "Ultra".into(),
        hardware_serial: Some("4C530001230321117283".into()),
        label: "BACKUP".into(),
        filesystem: "exFAT".into(),
    });
    t.ok(m::BACKUP_USB_TRUST, json!({"mount": stick.path().display().to_string()}), Some(&tok));
    let b = t.ok(m::BACKUP_CREATE, json!({}), Some(&tok));
    let name = b["file_name"].as_str().unwrap().to_string();
    let on_stick = crate::usb::backup_folder(stick.path()).join(&name).with_extension("rkey");
    assert!(on_stick.exists());

    let r = t.ok(m::BACKUP_REGENERATE_KEY, json!({"password": "owner-pass-1"}), Some(&tok));
    let new = r["recovery_key"].as_str().unwrap();
    assert_eq!(r["usb_updated"].as_u64(), Some(1));
    assert!(crate::dump::key_from_recovery(&on_stick, new).is_ok());

    // The stick is pulled out; a second regeneration cannot reach it, which the counts show.
    usb.0.lock().unwrap().clear();
    let again = t.ok(m::BACKUP_REGENERATE_KEY, json!({"password": "owner-pass-1"}), Some(&tok));
    assert_eq!(again["usb_updated"].as_u64(), Some(0));
    assert!(
        crate::dump::key_from_recovery(&on_stick, new).is_ok(),
        "the unreachable copy keeps the earlier key"
    );
    assert!(crate::dump::key_from_recovery(&on_stick, again["recovery_key"].as_str().unwrap()).is_err());
}

#[test]
fn a_wrong_password_is_refused_counted_and_changes_nothing() {
    let t = T::new();
    let key = t.setup();
    let tok = t.login("owner", "owner-pass-1");
    let before = fs::read(t.core.config().db_path().with_extension("rkey")).unwrap();
    for _ in 0..crate::auth::MAX_FAILED_ATTEMPTS {
        assert_eq!(
            t.call(m::BACKUP_REGENERATE_KEY, json!({"password": "not-the-password"}), Some(&tok)),
            Err(ErrorCode::InvalidCredentials)
        );
    }
    // Five wrong tries lock the account, exactly like login: even the right password is refused now.
    assert_eq!(
        t.call(m::BACKUP_REGENERATE_KEY, json!({"password": "owner-pass-1"}), Some(&tok)),
        Err(ErrorCode::AccountLocked)
    );
    assert_eq!(
        t.call(m::AUTH_LOGIN, json!({"username": "owner", "password": "owner-pass-1"}), None),
        Err(ErrorCode::AccountLocked)
    );
    assert_eq!(fs::read(t.core.config().db_path().with_extension("rkey")).unwrap(), before);
    assert!(rkey_opens(&t, &key));
}

#[test]
fn a_wrong_password_is_audited() {
    let t = T::new();
    t.setup();
    let tok = t.login("owner", "owner-pass-1");
    assert_eq!(
        t.call(m::BACKUP_REGENERATE_KEY, json!({"password": "nope"}), Some(&tok)),
        Err(ErrorCode::InvalidCredentials)
    );
    assert!(t.audit_actions(&tok).contains(&"backup.regenerate_key_failed".to_string()));
}

#[test]
fn only_the_owner_may_regenerate() {
    let t = T::new();
    let key = t.setup();
    let owner = t.login("owner", "owner-pass-1");
    for (user, role) in [("admin1", "administrator"), ("recep", "receptionist")] {
        t.ok(
            m::USERS_CREATE,
            json!({"username": user, "display_name": user, "password": "staff-pass-1", "role": role}),
            Some(&owner),
        );
        let tok = t.login(user, "staff-pass-1");
        assert_eq!(
            t.call(m::BACKUP_REGENERATE_KEY, json!({"password": "staff-pass-1"}), Some(&tok)),
            Err(ErrorCode::Forbidden),
            "{role}"
        );
    }
    assert_eq!(
        t.call(m::BACKUP_REGENERATE_KEY, json!({"password": "owner-pass-1"}), None),
        Err(ErrorCode::Unauthenticated)
    );
    assert!(rkey_opens(&t, &key), "nothing changed");
}
