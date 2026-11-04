//! Phase 10 through the public RPC entry point: backup destinations, restore, license read-only state.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use artaveo_shared::{methods as m, ErrorCode};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};

use crate::clock::{iso, now};
use crate::tests::T;
use crate::usb::{FakeUsb, UsbVolume};

fn volume(mount: &Path, serial: &str) -> UsbVolume {
    UsbVolume {
        mount: mount.to_path_buf(),
        vendor: "SanDisk".into(),
        product: "Ultra".into(),
        hardware_serial: Some(serial.into()),
        label: "BACKUP".into(),
        filesystem: "exFAT".into(),
    }
}

/// Backup file names have a one-second resolution.
fn pause() {
    std::thread::sleep(Duration::from_millis(1100));
}

fn kinds(t: &T, token: &str) -> Vec<String> {
    t.ok(m::NOTIFICATIONS_LIST, json!({}), Some(token))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap().to_string())
        .collect()
}

fn newest(t: &T, token: &str) -> Value {
    t.ok(m::BACKUP_HISTORY, json!({}), Some(token))[0].clone()
}

#[test]
fn a_trusted_usb_drive_gets_every_backup_and_a_missing_one_waits() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let stick = tempfile::tempdir().unwrap();
    let usb = Arc::new(FakeUsb::default());
    t.core.set_usb(usb.clone());
    usb.0.lock().unwrap().push(volume(stick.path(), "4C530001230321117283"));
    let mount = stick.path().display().to_string();

    // "New Backup Device Detected": nobody trusts it yet, so nothing is written there.
    let o = t.ok(m::BACKUP_USB, json!({}), Some(&owner));
    assert_eq!(o["volumes"][0]["verdict"], "new");
    assert!(o["trusted"].as_array().unwrap().is_empty());
    let first = t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    assert!(!stick.path().join("ArtaveoBackup").exists());
    assert!(newest(&t, &owner)["usb"].is_null());
    assert_eq!(t.ok(m::BACKUP_OVERVIEW, json!({}), Some(&owner))["usb"], "not_configured");

    // [Use for Backup]: a signed marker goes on the drive and the newest backup follows at once.
    let o = t.ok(m::BACKUP_USB_TRUST, json!({"mount": mount}), Some(&owner));
    assert_eq!(o["volumes"][0]["verdict"], "trusted");
    assert_eq!(o["trusted"][0]["connected"], true);
    assert_eq!(o["trusted"][0]["serial_hint"], "7283");
    let name = first["file_name"].as_str().unwrap();
    let folder = stick.path().join("ArtaveoBackup");
    assert!(folder.join(name).exists());
    assert!(folder.join(name).with_extension("rkey").exists(), "the key file travels with the backup");
    assert_eq!(newest(&t, &owner)["usb"], "done");

    // Unplugged: the next backup is made locally and waits for the drive (10.4).
    usb.0.lock().unwrap().clear();
    pause();
    let second = t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    assert_eq!(newest(&t, &owner)["usb"], "pending");
    let ov = t.ok(m::BACKUP_OVERVIEW, json!({}), Some(&owner));
    assert_eq!(
        (ov["usb"].as_str(), ov["usb_connected"].as_bool(), ov["usb_pending"].as_i64()),
        (Some("pending"), Some(false), Some(1))
    );
    assert!(kinds(&t, &owner).contains(&"usb_missing".to_string()));

    // Plugged in again: the scheduler's next tick transfers it by itself.
    usb.0.lock().unwrap().push(volume(stick.path(), "4C530001230321117283"));
    assert!(t.core.run_due_tasks().unwrap().is_none(), "no new backup is due");
    assert_eq!(newest(&t, &owner)["usb"], "done");
    assert!(folder.join(second["file_name"].as_str().unwrap()).exists());
    assert!(!kinds(&t, &owner).contains(&"usb_missing".to_string()));
    assert_eq!(t.ok(m::BACKUP_OVERVIEW, json!({}), Some(&owner))["usb_pending"], 0);

    // The marker copied onto another stick (other hardware serial) is not trusted.
    usb.0.lock().unwrap().clear();
    usb.0.lock().unwrap().push(volume(stick.path(), "4C530009999999999999"));
    let o = t.ok(m::BACKUP_USB, json!({}), Some(&owner));
    assert_eq!(o["volumes"][0]["verdict"], "cloned");
    assert_eq!(o["trusted"][0]["connected"], false);
    assert_eq!(
        t.call(m::BACKUP_USB_TRUST, json!({"mount": mount}), Some(&owner)),
        Err(ErrorCode::Validation)
    );
    assert!(kinds(&t, &owner).contains(&"usb_cloned".to_string()));

    // Forgetting a device removes the destination; unknown ids are refused.
    let id = o["trusted"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(
        t.call(m::BACKUP_USB_FORGET, json!({"device_id": "nope"}), Some(&owner)),
        Err(ErrorCode::NotFound)
    );
    let o = t.ok(m::BACKUP_USB_FORGET, json!({"device_id": id}), Some(&owner));
    assert!(o["trusted"].as_array().unwrap().is_empty());
    let actions = t.audit_actions(&owner);
    assert!(
        actions.contains(&"backup.usb_trust".to_string())
            && actions.contains(&"backup.usb_forget".to_string())
    );
}

#[test]
fn the_cloud_folder_gets_the_database_and_images_and_waits_while_missing() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("Google Drive");
    fs::create_dir(&folder).unwrap();
    // An encrypted image as the attachment store keeps it.
    let att = t.core.config().attachments_dir().join("ab");
    fs::create_dir_all(&att).unwrap();
    fs::write(att.join("abcd.bin"), b"encrypted image").unwrap();

    let mut s = t.ok(m::BACKUP_SETTINGS_GET, json!({}), Some(&owner));
    assert!(s["cloud_folder"].is_null());
    s["cloud_folder"] = json!(folder.display().to_string());
    t.ok(m::BACKUP_SETTINGS_SAVE, s.clone(), Some(&owner));
    let b = t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    let dir = folder.join("ArtaveoDental");
    assert!(dir.join(b["file_name"].as_str().unwrap()).exists());
    assert_eq!(fs::read(dir.join("attachments/ab/abcd.bin")).unwrap(), b"encrypted image");
    assert_eq!(newest(&t, &owner)["cloud"], "done");
    let ov = t.ok(m::BACKUP_OVERVIEW, json!({}), Some(&owner));
    assert_eq!(ov["cloud"], "ok");
    assert_eq!(ov["cloud_status"]["provider"], "google_drive");
    assert_eq!(ov["cloud_status"]["writable"], true);

    // Images are optional: with them off only the database goes.
    s["cloud_include_images"] = json!(false);
    t.ok(m::BACKUP_SETTINGS_SAVE, s.clone(), Some(&owner));
    fs::write(att.join("ffff.bin"), b"second image").unwrap();
    pause();
    t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    assert!(!dir.join("attachments/ab/ffff.bin").exists());

    // The folder is gone (drive not mounted): the backup stays local and waits.
    fs::remove_dir_all(&folder).unwrap();
    pause();
    t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    assert_eq!(newest(&t, &owner)["cloud"], "pending");
    assert_eq!(t.ok(m::BACKUP_OVERVIEW, json!({}), Some(&owner))["cloud"], "pending");
    fs::create_dir(&folder).unwrap();
    assert!(t.core.run_due_tasks().unwrap().is_none());
    assert_eq!(newest(&t, &owner)["cloud"], "done");
    assert!(folder.join("ArtaveoDental").read_dir().unwrap().count() >= 2);
}

#[test]
fn backup_settings_are_validated_and_shared_with_the_old_settings_page() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let base = t.ok(m::BACKUP_SETTINGS_GET, json!({}), Some(&owner));
    assert_eq!(
        (base["daily_hour"].as_u64(), base["daily_minute"].as_u64(), base["keep_daily"].as_u64()),
        (Some(19), Some(0), Some(14))
    );
    assert_eq!(
        (base["keep_weekly"].as_u64(), base["keep_monthly"].as_u64(), base["usb_warn_minutes"].as_u64()),
        (Some(4), Some(3), Some(30))
    );

    let bad = |field: &str, value: Value| {
        let mut s = base.clone();
        s[field] = value;
        t.call(m::BACKUP_SETTINGS_SAVE, s, Some(&owner))
    };
    for (field, value) in [
        ("daily_hour", json!(24)),
        ("daily_minute", json!(60)),
        ("usb_warn_minutes", json!(2)),
        ("keep_daily", json!(0)),
        ("keep_weekly", json!(105)),
        ("keep_monthly", json!(61)),
        ("cloud_keep_daily", json!(0)),
        ("cloud_folder", json!("relative/folder")),
        ("cloud_folder", json!("/definitely/not/there")),
        ("cloud_folder", json!(t.core.config().data_dir.display().to_string())),
    ] {
        assert_eq!(bad(field, value.clone()), Err(ErrorCode::Validation), "{field} = {value}");
    }

    let mut s = base.clone();
    s["daily_hour"] = json!(20);
    s["daily_minute"] = json!(30);
    s["keep_weekly"] = json!(8);
    let saved = t.ok(m::BACKUP_SETTINGS_SAVE, s, Some(&owner));
    assert_eq!(
        (saved["daily_hour"].as_u64(), saved["daily_minute"].as_u64(), saved["keep_weekly"].as_u64()),
        (Some(20), Some(30), Some(8))
    );
    // The older Settings page reads and writes the same hour.
    assert_eq!(t.ok(m::SETTINGS_GET, json!({}), Some(&owner))["daily_backup_hour"], 20);
    assert!(t.audit_actions(&owner).contains(&"backup.settings".to_string()));

    // Only people who may create backups can change the plan; viewing needs backup.view.
    t.ok(
        m::USERS_CREATE,
        json!({"username": "reception", "display_name": "پذیرش", "password": "recep-pass-1", "role": "receptionist"}),
        Some(&owner),
    );
    let rec = t.login("reception", "recep-pass-1");
    assert_eq!(t.call(m::BACKUP_SETTINGS_GET, json!({}), Some(&rec)), Err(ErrorCode::Forbidden));
    assert_eq!(t.call(m::BACKUP_USB_TRUST, json!({"mount": "E:\\"}), Some(&rec)), Err(ErrorCode::Forbidden));
}

#[test]
fn restore_validates_asks_the_owners_password_and_keeps_a_safety_copy() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad"}), Some(&owner));
    let b = t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    let name = b["file_name"].as_str().unwrap().to_string();
    t.ok(m::PATIENTS_CREATE, json!({"full_name": "Zarghuna"}), Some(&owner));

    let list = t.ok(m::BACKUP_RESTORE_LIST, json!({"source": "local"}), Some(&owner));
    assert_eq!(
        (list[0]["file_name"].as_str(), list[0]["has_key"].as_bool()),
        (Some(name.as_str()), Some(true))
    );
    let target = json!({"source": "local", "file_name": name});
    let pv = t.ok(m::BACKUP_RESTORE_PREVIEW, target.clone(), Some(&owner));
    assert_eq!(
        (pv["valid"].as_bool(), pv["patients"].as_i64(), pv["current_patients"].as_i64()),
        (Some(true), Some(1), Some(2))
    );
    assert_eq!(pv["clinic_name"], "کلینیک آزمایشی");

    // The password is asked again, and a file name can never be a path.
    let wrong = json!({"source": "local", "file_name": name, "password": "wrong-password"});
    assert_eq!(t.call(m::BACKUP_RESTORE, wrong, Some(&owner)), Err(ErrorCode::InvalidCredentials));
    assert_eq!(
        t.call(
            m::BACKUP_RESTORE_PREVIEW,
            json!({"source": "local", "file_name": "../clinic.db"}),
            Some(&owner)
        ),
        Err(ErrorCode::Validation)
    );
    // A damaged file can be previewed (and is reported) but never restored.
    let junk = "artaveo-20200101-000000-manual.db";
    fs::write(t.core.config().backup_dir().join(junk), b"not a database").unwrap();
    let pv = t.ok(m::BACKUP_RESTORE_PREVIEW, json!({"source": "local", "file_name": junk}), Some(&owner));
    assert_eq!((pv["valid"].as_bool(), pv["problem"].as_str()), (Some(false), Some("wrong_key")));
    let junk_restore = json!({"source": "local", "file_name": junk, "password": "owner-pass-1"});
    assert_eq!(t.call(m::BACKUP_RESTORE, junk_restore, Some(&owner)), Err(ErrorCode::BackupInvalid));
    assert!(t.core.is_set_up(), "a refused restore leaves the clinic untouched");

    // Only the Owner: an administrator has no backup.restore.
    t.ok(
        m::USERS_CREATE,
        json!({"username": "admin2", "display_name": "Admin", "password": "admin-pass-12", "role": "administrator"}),
        Some(&owner),
    );
    let admin = t.login("admin2", "admin-pass-12");
    let attempt = json!({"source": "local", "file_name": name, "password": "admin-pass-12"});
    assert_eq!(t.call(m::BACKUP_RESTORE, attempt, Some(&admin)), Err(ErrorCode::Forbidden));

    pause();
    let done = json!({"source": "local", "file_name": name, "password": "owner-pass-1"});
    let r = t.ok(m::BACKUP_RESTORE, done, Some(&owner));
    assert_eq!(r["patients"], 1);
    assert!(r["safety_backup"].as_str().unwrap().contains("prerestore"));
    // Every session ended: the data was replaced.
    assert_eq!(
        t.call(m::PATIENTS_LIST, json!({"limit": 50, "offset": 0}), Some(&owner)),
        Err(ErrorCode::SessionExpired)
    );
    let owner = t.login("owner", "owner-pass-1");
    let patients = t.ok(m::PATIENTS_LIST, json!({"limit": 50, "offset": 0}), Some(&owner));
    assert_eq!(patients["items"].as_array().unwrap().len(), 1);
    // The safety copy is in the register again and the restore is in the audit log.
    let backups = t.ok(m::BACKUP_LIST, json!({}), Some(&owner));
    assert!(backups
        .as_array()
        .unwrap()
        .iter()
        .any(|b| b["file_name"].as_str().unwrap().contains("prerestore")));
    assert!(t.audit_actions(&owner).contains(&"backup.restore".to_string()));
    // ... and the safety copy still holds the two patients: restoring it undoes the restore.
    let safety = r["safety_backup"].as_str().unwrap();
    pause();
    let undo = json!({"source": "local", "file_name": safety, "password": "owner-pass-1"});
    t.ok(m::BACKUP_RESTORE, undo, Some(&owner));
    let owner = t.login("owner", "owner-pass-1");
    let patients = t.ok(m::PATIENTS_LIST, json!({"limit": 50, "offset": 0}), Some(&owner));
    assert_eq!(patients["items"].as_array().unwrap().len(), 2);
}

#[test]
fn a_new_computer_restores_from_a_backup_with_the_recovery_key() {
    let old = T::new();
    let recovery_key = old.setup();
    let tok = old.login("owner", "owner-pass-1");
    old.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad"}), Some(&tok));
    let b = old.ok(m::BACKUP_CREATE, json!({}), Some(&tok));
    let name = b["file_name"].as_str().unwrap().to_string();
    let usb_like = tempfile::tempdir().unwrap();
    let from = old.core.config().backup_dir().join(&name);
    fs::copy(&from, usb_like.path().join(&name)).unwrap();
    fs::copy(from.with_extension("rkey"), usb_like.path().join(&name).with_extension("rkey")).unwrap();
    let folder = usb_like.path().display().to_string();

    let new = T::new();
    assert!(!new.core.is_set_up());
    let found = new.ok(m::BACKUP_SCAN, json!({"folder": folder}), None);
    assert_eq!(
        (found[0]["file_name"].as_str(), found[0]["has_key"].as_bool()),
        (Some(name.as_str()), Some(true))
    );

    let other_key = crate::keys::RecoveryKey::generate().to_display();
    let wrong = json!({"folder": folder, "file_name": name, "recovery_key": other_key});
    assert_eq!(new.call(m::BACKUP_RESTORE_NEW, wrong, None), Err(ErrorCode::RecoveryKeyInvalid));
    assert!(!new.core.is_set_up());
    let right = json!({"folder": folder, "file_name": name, "recovery_key": recovery_key});
    new.ok(m::BACKUP_RESTORE_NEW, right.clone(), None);
    assert!(new.core.is_set_up());

    let tok = new.login("owner", "owner-pass-1");
    assert_eq!(
        new.ok(m::PATIENTS_LIST, json!({"limit": 50, "offset": 0}), Some(&tok))["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(new.audit_actions(&tok).contains(&"backup.restore_new_computer".to_string()));
    // The clinic opens normally on the next start (the key is protected for this computer now).
    let new = new.reopen();
    assert!(new.core.is_set_up());
    // A computer that already has a clinic never takes this path.
    assert_eq!(new.call(m::BACKUP_RESTORE_NEW, right, None), Err(ErrorCode::AlreadySetUp));
    assert_eq!(new.call(m::BACKUP_SCAN, json!({"folder": null}), None), Err(ErrorCode::AlreadySetUp));
}

// ───────────────────────────── license ─────────────────────────────

/// Writes a token as the Control Center would sign it for this very installation and hardware.
#[allow(clippy::too_many_arguments)]
fn write_license(
    t: &T,
    key: &SigningKey,
    result: &str,
    computers: u32,
    issued_days_ago: i64,
    expires_in_days: i64,
    valid_until_in_days: i64,
    check_every_days: i64,
) {
    let at = now();
    let day = time::Duration::days;
    let payload = json!({
        "result": result, "kind": "paid", "plan": "standard", "max_computers": computers,
        "installation_id": t.core.license.installation_id(), "fingerprint": t.core.license.fingerprint(),
        "issued_at": iso(at - day(issued_days_ago)), "expires_at": iso(at + day(expires_in_days)),
        "check_every_days": check_every_days, "offline_grace_days": 120, "post_expiry_grace_days": 120,
        "valid_until": iso(at + day(valid_until_in_days)),
    });
    let b64 = |b: &[u8]| data_encoding::BASE64URL_NOPAD.encode(b);
    let header = b64(json!({"alg": "EdDSA", "kid": "t"}).to_string().as_bytes());
    let body = b64(payload.to_string().as_bytes());
    let signature = b64(&key.sign(format!("{header}.{body}").as_bytes()).to_bytes());
    fs::write(
        t.core.config().data_dir.join("license.json"),
        json!({ "token": format!("{header}.{body}.{signature}") }).to_string(),
    )
    .unwrap();
    t.core.license.set_keys(vec![("t".into(), key.verifying_key().to_bytes())]);
}

#[test]
fn the_license_states_decide_what_can_be_changed_and_never_hide_the_data() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let key = SigningKey::from_bytes(&[3u8; 32]);
    let status = |t: &T| t.ok(m::LICENSE_STATUS, json!({}), Some(&owner));

    // Development and tests work without a license; a production build would be read-only here.
    let s = status(&t);
    assert_eq!((s["state"].as_str(), s["read_only"].as_bool()), (Some("unactivated"), Some(false)));
    assert_eq!(s["fingerprint_hint"].as_str().unwrap().len(), 9);
    t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad"}), Some(&owner));

    // The vendor's grace is over: read-only.
    write_license(&t, &key, "valid", 3, 100, -10, -1, 30);
    let s = status(&t);
    assert_eq!((s["state"].as_str(), s["read_only"].as_bool()), (Some("expired"), Some(true)));
    assert_eq!(
        t.call(m::PATIENTS_CREATE, json!({"full_name": "Zarghuna"}), Some(&owner)),
        Err(ErrorCode::LicenseReadOnly)
    );
    assert_eq!(
        t.call(m::SETTINGS_UPDATE, t.ok(m::SETTINGS_GET, json!({}), Some(&owner)), Some(&owner)),
        Err(ErrorCode::LicenseReadOnly)
    );
    // Everything that only looks, prints, exports or protects the data keeps working.
    let list = t.ok(m::PATIENTS_LIST, json!({"limit": 50, "offset": 0}), Some(&owner));
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    t.ok(m::PATIENTS_EXPORT, json!({}), Some(&owner));
    assert_eq!(t.ok(m::DASHBOARD_GET, json!({}), Some(&owner))["license"], "expired");
    assert!(kinds(&t, &owner).contains(&"license_read_only".to_string()));

    // Past the end date but inside the grace: it works, with a warning.
    write_license(&t, &key, "valid", 3, 5, -10, 50, 30);
    let s = status(&t);
    assert_eq!(
        (s["state"].as_str(), s["read_only"].as_bool(), s["days_left"].as_i64()),
        (Some("expired_warning"), Some(false), Some(-10))
    );
    t.ok(m::PATIENTS_CREATE, json!({"full_name": "Zarghuna"}), Some(&owner));
    assert!(kinds(&t, &owner).contains(&"license_expired".to_string()));

    // A renewal (a fresh token) lifts the warning at once.
    write_license(&t, &key, "valid", 3, 0, 300, 120, 30);
    let s = status(&t);
    assert_eq!(
        (s["state"].as_str(), s["read_only"].as_bool(), s["plan"].as_str()),
        (Some("active"), Some(false), Some("standard"))
    );
    assert_eq!(s["days_left"], 300);
    assert!(!kinds(&t, &owner).contains(&"license_expired".to_string()));

    // The monthly check is late: it still works, with a reminder; with no check until the token's limit it is read-only.
    write_license(&t, &key, "valid", 3, 40, 300, 80, 30);
    let s = status(&t);
    assert_eq!(
        (s["state"].as_str(), s["read_only"].as_bool(), s["verify_due"].as_bool()),
        (Some("grace"), Some(false), Some(true))
    );
    assert!(kinds(&t, &owner).contains(&"license_verify".to_string()));
    write_license(&t, &key, "valid", 3, 200, 300, -10, 30);
    assert_eq!(status(&t)["state"], "needs_verification");
    assert_eq!(
        t.call(m::PATIENTS_CREATE, json!({"full_name": "X"}), Some(&owner)),
        Err(ErrorCode::LicenseReadOnly)
    );

    // The vendor has not approved this computer / has blocked it / has suspended the license.
    for (result, state) in [("pending", "pending"), ("blocked", "blocked"), ("license_suspended", "revoked")]
    {
        write_license(&t, &key, result, 3, 0, 300, 120, 30);
        let s = status(&t);
        assert_eq!((s["state"].as_str(), s["read_only"].as_bool()), (Some(state), Some(true)), "{result}");
    }
}

#[test]
fn the_plan_sets_the_lan_client_limit_and_only_managers_use_the_license_pages() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let key = SigningKey::from_bytes(&[4u8; 32]);
    write_license(&t, &key, "valid", 3, 0, 200, 120, 30);
    t.core.apply_license_limits();
    // 3 computers = the clinic server and 2 more.
    assert_eq!(t.ok(m::LAN_STATUS, json!({}), Some(&owner))["client_limit"], 2);

    // A key must look like a key before anything is sent; this build has no license server address.
    let bad = json!({"code": "ARTA-AAAA-BBBB-CCCC"});
    assert_eq!(t.call(m::LICENSE_ACTIVATE, bad, Some(&owner)), Err(ErrorCode::Validation));
    let good = json!({"code": "0A1B-2C3D-4E5F-6A7B-8C9D-0E1F"});
    assert_eq!(
        t.call(m::LICENSE_ACTIVATE, good.clone(), Some(&owner)),
        Err(ErrorCode::LicenseServerUnavailable)
    );
    assert_eq!(
        t.call(m::LICENSE_VERIFY, json!({}), Some(&owner)),
        Err(ErrorCode::LicenseRejected),
        "no key was ever accepted"
    );
    assert_eq!(
        t.call(m::LICENSE_OFFLINE_APPLY, json!({"response_code": "xxx"}), Some(&owner)),
        Err(ErrorCode::LicenseRejected)
    );
    let req = t.ok(m::LICENSE_OFFLINE_REQUEST, json!({}), Some(&owner));
    assert!(req["request_code"].as_str().unwrap().len() > 60);

    t.ok(
        m::USERS_CREATE,
        json!({"username": "reception", "display_name": "پذیرش", "password": "recep-pass-1", "role": "receptionist"}),
        Some(&owner),
    );
    let rec = t.login("reception", "recep-pass-1");
    assert!(
        t.ok(m::LICENSE_STATUS, json!({}), Some(&rec))["state"].is_string(),
        "everyone may read the state"
    );
    assert_eq!(t.call(m::LICENSE_ACTIVATE, good, Some(&rec)), Err(ErrorCode::Forbidden));
    assert_eq!(t.call(m::LICENSE_VERIFY, json!({}), Some(&rec)), Err(ErrorCode::Forbidden));
}

#[test]
fn the_license_survives_a_restore_so_an_old_backup_cannot_bring_back_a_trial() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let b = t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    let key = SigningKey::from_bytes(&[5u8; 32]);
    write_license(&t, &key, "valid", 1, 100, -10, -1, 30); // read-only: the vendor's grace is over
    pause();
    let restore = json!({"source": "local", "file_name": b["file_name"], "password": "owner-pass-1"});
    // Backup and restore keep working in read-only state (10.9) ...
    t.ok(m::BACKUP_RESTORE, restore, Some(&owner));
    let owner = t.login("owner", "owner-pass-1");
    // ... and the token beside the database was not touched by it.
    assert_eq!(t.ok(m::LICENSE_STATUS, json!({}), Some(&owner))["state"], "expired");
}
