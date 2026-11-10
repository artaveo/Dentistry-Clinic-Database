//! Phase 11 through the public RPC entry point: support report and the safety copy before an update.

use std::fs;

use artaveo_shared::{methods as m, ErrorCode};
use serde_json::json;

use crate::tests::T;

#[test]
fn the_support_report_is_written_on_request_audited_and_limited_to_managers() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    t.ok(
        m::USERS_CREATE,
        json!({"username": "reception", "display_name": "پذیرش", "password": "recep-pass-1", "role": "receptionist"}),
        Some(&owner),
    );
    let rec = t.login("reception", "recep-pass-1");
    assert_eq!(t.call(m::SYSTEM_REPORT, json!({}), Some(&rec)), Err(ErrorCode::Forbidden));

    let r = t.ok(m::SYSTEM_REPORT, json!({}), Some(&owner));
    let path = std::path::PathBuf::from(r["path"].as_str().unwrap());
    assert_eq!(path.parent().unwrap(), t.core.config().exports_dir());
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("app_version:") && text.contains("schema_version:") && text.contains("integrity:"));
    assert!(text.contains("license_state:") && text.contains("last_backup:"));
    assert_eq!(r["bytes"].as_i64().unwrap() as usize, text.len());
    assert!(t.audit_actions(&owner).contains(&"support.report".to_string()));
}

#[test]
fn the_report_holds_no_patient_data() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    t.ok(
        m::PATIENTS_CREATE,
        json!({"full_name": "زرمینه احمدی", "phone": "0700123456"}),
        Some(&owner),
    );
    let r = t.ok(m::SYSTEM_REPORT, json!({}), Some(&owner));
    let text = fs::read_to_string(r["path"].as_str().unwrap()).unwrap();
    assert!(!text.contains("زرمینه") && !text.contains("احمدی") && !text.contains("0700123456"));
}

#[test]
fn the_safety_copy_before_an_update_is_a_valid_backup_that_retention_keeps() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let info = t.ok(m::UPDATE_PREPARE, json!({}), Some(&owner));
    let name = info["file_name"].as_str().unwrap();
    assert!(name.contains("-preupdate."), "{name}");
    assert_eq!(info["verified"], true);
    let dir = t.core.config().backup_dir();
    assert!(dir.join(name).exists() && dir.join(name).with_extension("rkey").exists());
    let listed = t.ok(m::BACKUP_LIST, json!({}), Some(&owner));
    assert!(listed.as_array().unwrap().iter().any(|b| b["file_name"] == name));

    t.ok(
        m::USERS_CREATE,
        json!({"username": "reception", "display_name": "پذیرش", "password": "recep-pass-1", "role": "receptionist"}),
        Some(&owner),
    );
    let rec = t.login("reception", "recep-pass-1");
    assert_eq!(t.call(m::UPDATE_PREPARE, json!({}), Some(&rec)), Err(ErrorCode::Forbidden));
}
