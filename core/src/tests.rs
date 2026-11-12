//! End-to-end tests of the Core through its public RPC entry point, plus
//! a few checks that need internal access (append-only audit, migrations).

use std::time::Duration;

use artaveo_shared::{methods as m, ErrorCode, RpcRequest, RpcResponse};
use serde_json::{json, Value};

use crate::keys::{default_protector, RecoveryKey};
use crate::{db, Config, Core, Environment};

pub(crate) struct T {
    pub(crate) _dir: tempfile::TempDir,
    pub(crate) core: Core,
}

impl T {
    pub(crate) fn new() -> Self {
        // Booking tests use 09:00 "today": pin the clock to 06:00 clinic time so they are never in the past.
        let six = crate::clock::local_now().replace_time(time::macros::time!(06:00));
        crate::clock::pin_booking_clock(six);
        let dir = tempfile::tempdir().unwrap();
        let core = Core::open(Config::new(Environment::Test, dir.path().to_path_buf())).unwrap();
        T { _dir: dir, core }
    }

    pub(crate) fn reopen(self) -> Self {
        let dir = self._dir;
        drop(self.core);
        let core = Core::open(Config::new(Environment::Test, dir.path().to_path_buf())).unwrap();
        T { _dir: dir, core }
    }

    pub(crate) fn call(&self, method: &str, params: Value, token: Option<&str>) -> Result<Value, ErrorCode> {
        match self.core.handle(RpcRequest { method: method.into(), params, token: token.map(Into::into) }) {
            RpcResponse::Ok { result } => Ok(result),
            RpcResponse::Error { error } => Err(error.code),
        }
    }

    pub(crate) fn ok(&self, method: &str, params: Value, token: Option<&str>) -> Value {
        match self.core.handle(RpcRequest { method: method.into(), params, token: token.map(Into::into) }) {
            RpcResponse::Ok { result } => result,
            // The detail says which step failed (the code alone is just "internal").
            RpcResponse::Error { error } => panic!("{method} failed: {:?}: {}", error.code, error.detail),
        }
    }

    pub(crate) fn setup(&self) -> String {
        let r = self.ok(
            m::APP_SETUP,
            json!({
                "clinic_name": "کلینیک آزمایشی", "owner_username": "owner", "owner_display_name": "مالک",
                "owner_password": "owner-pass-1", "language": "fa"
            }),
            None,
        );
        r["recovery_key"].as_str().unwrap().to_string()
    }

    pub(crate) fn login(&self, user: &str, pass: &str) -> String {
        self.ok(m::AUTH_LOGIN, json!({"username": user, "password": pass}), None)["token"]
            .as_str()
            .unwrap()
            .to_string()
    }

    pub(crate) fn audit_actions(&self, token: &str) -> Vec<String> {
        self.ok(m::AUDIT_LIST, json!({"limit": 500, "offset": 0}), Some(token))
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["action"].as_str().unwrap().to_string())
            .collect()
    }
}

#[test]
fn first_run_setup_then_ready() {
    let t = T::new();
    assert_eq!(t.ok(m::APP_STATUS, json!({}), None)["state"], "needs_setup");
    assert_eq!(
        t.call(m::AUTH_LOGIN, json!({"username": "a", "password": "b"}), None),
        Err(ErrorCode::NotSetUp)
    );
    // Weak password rejected and nothing left behind.
    assert_eq!(
        t.call(m::APP_SETUP, json!({"clinic_name": "C", "owner_username": "owner", "owner_display_name": "O", "owner_password": "short", "language": "fa"}), None),
        Err(ErrorCode::Validation)
    );
    let rk = t.setup();
    assert_eq!(rk.len(), 41, "6 groups of 6 + dashes: {rk}");
    let st = t.ok(m::APP_STATUS, json!({}), None);
    assert_eq!(st["state"], "ready");
    assert_eq!(st["clinic_name"], "کلینیک آزمایشی");
    assert_eq!(st["version"], crate::APP_VERSION);
    assert_eq!(t.call(m::APP_SETUP, json!({"clinic_name": "X", "owner_username": "owner2", "owner_display_name": "O", "owner_password": "owner-pass-1", "language": "fa"}), None), Err(ErrorCode::AlreadySetUp));

    let tok = t.login("OWNER", "owner-pass-1"); // usernames are case-insensitive
    let actions = t.audit_actions(&tok);
    assert!(actions.contains(&"app.setup".into()) && actions.contains(&"auth.login".into()));
    let bytes = std::fs::read(t.core.config().db_path()).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3"), "database file must be encrypted");
}

#[test]
fn login_failures_lock_the_account_and_are_audited() {
    let t = T::new();
    t.setup();
    assert_eq!(
        t.call(m::AUTH_LOGIN, json!({"username": "nobody", "password": "x"}), None),
        Err(ErrorCode::InvalidCredentials)
    );
    for _ in 0..crate::auth::MAX_FAILED_ATTEMPTS {
        assert_eq!(
            t.call(m::AUTH_LOGIN, json!({"username": "owner", "password": "wrong"}), None),
            Err(ErrorCode::InvalidCredentials)
        );
    }
    assert_eq!(
        t.call(m::AUTH_LOGIN, json!({"username": "owner", "password": "owner-pass-1"}), None),
        Err(ErrorCode::AccountLocked)
    );
}

#[test]
fn permissions_are_enforced_by_the_core() {
    let t = T::new();
    t.setup();
    assert_eq!(t.call(m::USERS_LIST, json!({}), None), Err(ErrorCode::Unauthenticated));
    assert_eq!(t.call(m::USERS_LIST, json!({}), Some("forged")), Err(ErrorCode::SessionExpired));
    assert_eq!(t.call("patients.delete_everything", json!({}), Some("x")), Err(ErrorCode::UnknownMethod));
    let owner = t.login("owner", "owner-pass-1");
    let roles = t.ok(m::ROLES_LIST, json!({}), Some(&owner));
    assert_eq!(roles.as_array().unwrap().len(), 6);

    t.ok(m::USERS_CREATE, json!({"username": "reception", "display_name": "پذیرش", "password": "recep-pass-1", "role": "receptionist"}), Some(&owner));
    assert_eq!(
        t.call(
            m::USERS_CREATE,
            json!({"username": "boss2", "display_name": "x", "password": "boss-pass-12", "role": "owner"}),
            Some(&owner)
        ),
        Err(ErrorCode::Validation),
        "only one owner"
    );
    let rec = t.login("reception", "recep-pass-1");
    assert_eq!(t.call(m::USERS_LIST, json!({}), Some(&rec)), Err(ErrorCode::Forbidden));
    assert_eq!(t.call(m::BACKUP_CREATE, json!({}), Some(&rec)), Err(ErrorCode::Forbidden));
    assert_eq!(
        t.call(m::AUDIT_LIST, json!({"limit": 1, "offset": 0}), Some(&rec)),
        Err(ErrorCode::Forbidden)
    );
    assert!(t.call(m::SYSTEM_INFO, json!({}), Some(&rec)).is_ok());
}

#[test]
fn optimistic_locking_detects_concurrent_edits() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let u = t.ok(m::USERS_CREATE, json!({"username": "dr.zarghona", "display_name": "داکتر زرغونه", "password": "doctor-pass-1", "role": "doctor"}), Some(&owner));
    let id = u["id"].as_str().unwrap();
    let v = u["version"].as_i64().unwrap();
    let first = t.ok(
        m::USERS_UPDATE,
        json!({"id": id, "version": v, "display_name": "Dr. Zarghona", "role": "doctor", "is_active": true}),
        Some(&owner),
    );
    assert_eq!(first["version"], v + 1);
    // A second editor still holding the old version is refused.
    assert_eq!(
        t.call(
            m::USERS_UPDATE,
            json!({"id": id, "version": v, "display_name": "Other", "role": "assistant", "is_active": true}),
            Some(&owner)
        ),
        Err(ErrorCode::Conflict)
    );
    // Deactivation ends the user's sessions.
    let doc = t.login("dr.zarghona", "doctor-pass-1");
    t.ok(m::USERS_UPDATE, json!({"id": id, "version": v + 1, "display_name": "Dr. Zarghona", "role": "doctor", "is_active": false}), Some(&owner));
    assert_eq!(t.call(m::SYSTEM_INFO, json!({}), Some(&doc)), Err(ErrorCode::SessionExpired));
    assert_eq!(
        t.call(m::AUTH_LOGIN, json!({"username": "dr.zarghona", "password": "doctor-pass-1"}), None),
        Err(ErrorCode::InvalidCredentials)
    );
}

#[test]
fn lock_screen_manual_and_idle() {
    let t = T::new();
    t.setup();
    let tok = t.login("owner", "owner-pass-1");
    assert_eq!(t.ok(m::SESSION_LOCK, json!({}), Some(&tok))["locked"], true);
    assert_eq!(t.call(m::USERS_LIST, json!({}), Some(&tok)), Err(ErrorCode::SessionLocked));
    assert_eq!(
        t.call(m::SESSION_UNLOCK, json!({"password": "nope"}), Some(&tok)),
        Err(ErrorCode::InvalidCredentials)
    );
    assert_eq!(t.ok(m::SESSION_UNLOCK, json!({"password": "owner-pass-1"}), Some(&tok))["locked"], false);
    t.ok(m::USERS_LIST, json!({}), Some(&tok));

    // Idle past the timeout → locked automatically.
    t.core.lock_sessions().age(&tok, Duration::from_secs(11 * 60));
    assert_eq!(t.ok(m::SESSION_STATE, json!({}), Some(&tok))["locked"], true);
    assert_eq!(t.call(m::USERS_LIST, json!({}), Some(&tok)), Err(ErrorCode::SessionLocked));
    let actions = {
        t.ok(m::SESSION_UNLOCK, json!({"password": "owner-pass-1"}), Some(&tok));
        t.audit_actions(&tok)
    };
    for a in ["session.lock", "session.unlock_failed", "session.unlock"] {
        assert!(actions.contains(&a.to_string()), "{a}");
    }
    // Shorter configured timeout applies immediately.
    let mut s = t.ok(m::SETTINGS_GET, json!({}), Some(&tok));
    s["session_timeout_minutes"] = json!(1);
    t.ok(m::SETTINGS_UPDATE, s.clone(), Some(&tok));
    t.core.lock_sessions().age(&tok, Duration::from_secs(61));
    assert_eq!(t.call(m::USERS_LIST, json!({}), Some(&tok)), Err(ErrorCode::SessionLocked));
    s["session_timeout_minutes"] = json!(0);
    t.ok(m::SESSION_UNLOCK, json!({"password": "owner-pass-1"}), Some(&tok));
    assert_eq!(t.call(m::SETTINGS_UPDATE, s, Some(&tok)), Err(ErrorCode::Validation));
}

#[test]
fn owner_password_recovery_with_recovery_key() {
    let t = T::new();
    let rk = t.setup();
    let old = t.login("owner", "owner-pass-1");
    let wrong = RecoveryKey::generate().to_display();
    assert_eq!(
        t.call(m::AUTH_RECOVER_OWNER, json!({"recovery_key": wrong, "new_password": "brand-new-pass"}), None),
        Err(ErrorCode::RecoveryKeyInvalid)
    );
    assert_eq!(
        t.call(
            m::AUTH_RECOVER_OWNER,
            json!({"recovery_key": "garbage", "new_password": "brand-new-pass"}),
            None
        ),
        Err(ErrorCode::RecoveryKeyInvalid)
    );
    t.ok(
        m::AUTH_RECOVER_OWNER,
        json!({"recovery_key": rk.to_lowercase(), "new_password": "brand-new-pass"}),
        None,
    );
    assert_eq!(
        t.call(m::SYSTEM_INFO, json!({}), Some(&old)),
        Err(ErrorCode::SessionExpired),
        "old sessions end"
    );
    assert_eq!(
        t.call(m::AUTH_LOGIN, json!({"username": "owner", "password": "owner-pass-1"}), None),
        Err(ErrorCode::InvalidCredentials)
    );
    let tok = t.login("owner", "brand-new-pass");
    let actions = t.audit_actions(&tok);
    assert!(
        actions.contains(&"auth.recover_owner".into())
            && actions.contains(&"auth.recover_owner_failed".into())
    );
}

#[test]
fn reference_and_geo_data_in_three_languages() {
    let t = T::new();
    t.setup();
    let tok = t.login("owner", "owner-pass-1");
    let labels = |lang: &str| -> Vec<String> {
        t.ok(m::REFERENCE_LIST, json!({"type_code": "gender", "language": lang}), Some(&tok))
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["label"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(labels("fa"), ["مرد", "زن"]);
    assert_eq!(labels("ps"), ["نارینه", "ښځینه"]);
    assert_eq!(labels("en"), ["Male", "Female"]);
    let fa = t.ok(m::GEO_PROVINCES, json!({"language": "fa"}), Some(&tok));
    let en = t.ok(m::GEO_PROVINCES, json!({"language": "en"}), Some(&tok));
    assert_eq!(fa.as_array().unwrap().len(), 34);
    let kabul_fa = fa.as_array().unwrap().iter().find(|p| p["code"] == "AF-KAB").unwrap();
    let kabul_en = en.as_array().unwrap().iter().find(|p| p["code"] == "AF-KAB").unwrap();
    assert_eq!(kabul_fa["id"], kabul_en["id"], "one id, many labels");
    assert_eq!(kabul_fa["label"], "کابل");
    assert_eq!(kabul_en["label"], "Kabul");
    assert_eq!(kabul_fa["id"], crate::ids::seed_id("province", "AF-KAB"), "same id in every clinic");
    let districts = |lang: &str| -> Vec<String> {
        t.ok(m::GEO_DISTRICTS, json!({"province_id": kabul_fa["id"], "language": lang}), Some(&tok))
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["label"].as_str().unwrap().to_string())
            .collect()
    };
    let fa_d = districts("fa");
    assert_eq!(fa_d.len(), 15, "Kabul: centre + 14 districts");
    assert_eq!(fa_d[0], "شهر کابل", "provincial centre first");
    assert!(fa_d.contains(&"پغمان".to_string()));
    assert!(districts("ps").contains(&"پغمان".to_string()));
    assert!(districts("en").contains(&"Paghman".to_string()));
}

#[test]
fn backup_is_encrypted_verified_and_restorable_with_recovery_key() {
    let t = T::new();
    let rk = t.setup();
    let tok = t.login("owner", "owner-pass-1");
    let b = t.ok(m::BACKUP_CREATE, json!({}), Some(&tok));
    assert_eq!(b["verified"], true);
    assert_eq!(b["kind"], "manual");
    let path = t.core.config().backup_dir().join(b["file_name"].as_str().unwrap());
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3"));
    // New computer: only the backup + its .rkey + the printed Recovery Key.
    let key = RecoveryKey::parse(&rk)
        .unwrap()
        .unwrap(&std::fs::read(path.with_extension("rkey")).unwrap())
        .unwrap();
    let conn = db::connect(&path, &key).unwrap();
    let name: String =
        conn.query_row("SELECT value FROM db_meta WHERE key='clinic_name'", [], |r| r.get(0)).unwrap();
    assert_eq!(name, "کلینیک آزمایشی");

    assert_eq!(t.ok(m::BACKUP_LIST, json!({}), Some(&tok)).as_array().unwrap().len(), 1);
    assert!(t.core.run_due_tasks().unwrap().is_none(), "a backup was just made");
    let info = t.ok(m::SYSTEM_INFO, json!({}), Some(&tok));
    assert_eq!(info["last_backup"]["id"], b["id"]);
    assert_eq!(info["database"]["encrypted"], true);
}

#[test]
fn backup_retention_keeps_the_newest_three_whatever_the_settings() {
    let t = T::new();
    t.setup();
    let tok = t.login("owner", "owner-pass-1");
    let mut s = t.ok(m::SETTINGS_GET, json!({}), Some(&tok));
    s["backup_keep_daily"] = json!(1);
    t.ok(m::SETTINGS_UPDATE, s, Some(&tok));
    let mut names = vec![];
    for _ in 0..4 {
        names.push(t.ok(m::BACKUP_CREATE, json!({}), Some(&tok))["file_name"].as_str().unwrap().to_string());
        std::thread::sleep(Duration::from_millis(1100)); // file names have 1 s resolution
    }
    // One per day would leave a single copy; the newest three always stay (10.5).
    let listed = t.ok(m::BACKUP_LIST, json!({}), Some(&tok));
    assert_eq!(listed.as_array().unwrap().len(), 3);
    assert!(!t.core.config().backup_dir().join(&names[0]).exists());
    assert!(t.core.config().backup_dir().join(&names[3]).exists());
}

#[test]
fn setup_with_full_wizard_fields_stores_the_clinic_profile() {
    let t = T::new();
    let kabul = crate::ids::seed_id("province", "AF-KAB");
    t.ok(
        m::APP_SETUP,
        json!({
            "clinic_name": "کلینیک کامل", "owner_username": "owner", "owner_display_name": "مالک",
            "owner_password": "owner-pass-1", "language": "fa",
            "install_mode": "single", "province_id": kabul, "address": "کابل، سرک سوم",
            "phone": "0700000000", "calendar_system": "gregorian", "clinic_mode": "solo",
            "theme": "dark", "color_primary": "#112233", "color_secondary": "#445566",
            "color_accent": "#778899", "working_hours": [{"day": 0, "closed": false, "open": "08:00", "close": "16:00"}],
            "trial_acknowledged": true
        }),
        None,
    );
    let tok = t.login("owner", "owner-pass-1");
    let c = t.ok(m::CLINIC_GET, json!({}), Some(&tok));
    assert_eq!(c["name"], "کلینیک کامل");
    assert_eq!(c["province_id"], kabul);
    assert_eq!(c["calendar_system"], "gregorian");
    assert_eq!(c["theme"], "dark");
    assert_eq!(c["color_primary"], "#112233");
    assert_eq!(c["working_hours"][0]["open"], "08:00");
    assert!(c["trial_started_at"].is_string());
}

#[test]
fn clinic_get_has_defaults_and_update_is_audited_and_permissioned() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let before = t.ok(m::CLINIC_GET, json!({}), Some(&owner));
    assert_eq!(before["calendar_system"], "shamsi");
    assert_eq!(before["clinic_mode"], "solo");
    assert_eq!(before["theme"], "system");
    assert_eq!(before["working_hours"], json!([]));

    t.ok(
        m::USERS_CREATE,
        json!({"username": "reception", "display_name": "پذیرش", "password": "recep-pass-1", "role": "receptionist"}),
        Some(&owner),
    );
    let rec = t.login("reception", "recep-pass-1");
    let mut edited = before.clone();
    edited["address"] = json!("هرات");
    edited["theme"] = json!("dark");
    edited["color_primary"] = json!("#abcdef");
    assert_eq!(t.call(m::CLINIC_UPDATE, edited.clone(), Some(&rec)), Err(ErrorCode::Forbidden));

    let after = t.ok(m::CLINIC_UPDATE, edited, Some(&owner));
    assert_eq!(after["address"], "هرات");
    assert_eq!(after["theme"], "dark");
    assert_eq!(after["color_primary"], "#abcdef");
    assert_eq!(after["name"], before["name"], "name is not editable through clinic.update");
    assert!(t.audit_actions(&owner).contains(&"clinic.update".to_string()));

    let mut bad_color = after.clone();
    bad_color["color_primary"] = json!("not-a-color");
    assert_eq!(t.call(m::CLINIC_UPDATE, bad_color, Some(&owner)), Err(ErrorCode::Validation));
}

#[test]
fn perio_and_stock_module_settings_default_on_and_round_trip() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let mut s = t.ok(m::SETTINGS_GET, json!({}), Some(&owner));
    assert_eq!(s["show_perio_chart"], true);
    assert_eq!(s["use_stock_module"], true);

    s["show_perio_chart"] = json!(false);
    s["use_stock_module"] = json!(false);
    let saved = t.ok(m::SETTINGS_UPDATE, s, Some(&owner));
    assert_eq!(saved["show_perio_chart"], false);
    assert_eq!(saved["use_stock_module"], false);
    let again = t.ok(m::SETTINGS_GET, json!({}), Some(&owner));
    assert_eq!(again["show_perio_chart"], false);
    assert_eq!(again["use_stock_module"], false);

    // An old client that does not send the new fields leaves the clinic with both on.
    let mut old = again.clone();
    old.as_object_mut().unwrap().remove("show_perio_chart");
    old.as_object_mut().unwrap().remove("use_stock_module");
    let back = t.ok(m::SETTINGS_UPDATE, old, Some(&owner));
    assert_eq!(back["show_perio_chart"], true);
    assert_eq!(back["use_stock_module"], true);
}

#[test]
fn reopen_persists_and_background_integrity_check_passes() {
    let t = T::new();
    t.setup();
    let t = t.reopen();
    assert!(t.core.is_set_up());
    let tok = t.login("owner", "owner-pass-1");
    for _ in 0..200 {
        if t.core.integrity().status != "running" {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(t.core.integrity().status, "ok");
    let info = t.ok(m::SYSTEM_INFO, json!({}), Some(&tok));
    assert_eq!(info["database"]["schema_version"], db::MIGRATIONS.len());
    assert!(info["database"]["cipher_version"].as_str().unwrap().starts_with('4'));
    assert_eq!(info["build_arch"], crate::sysinfo::build_arch());
}

#[test]
fn audit_log_is_append_only_in_the_database() {
    let t = T::new();
    t.setup();
    t.core
        .with_db(|o| {
            assert!(o.conn.execute("UPDATE audit_log SET action = 'x'", []).is_err());
            assert!(o.conn.execute("DELETE FROM audit_log", []).is_err());
            Ok(())
        })
        .unwrap();
}

#[test]
fn upgrade_from_older_schema_backs_up_migrates_and_audits() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::new(Environment::Test, dir.path().to_path_buf());
    // An "old release" database that only knows migration 1.
    {
        let (mut conn, _key) =
            db::create(&config.db_path(), default_protector().as_ref(), &RecoveryKey::generate()).unwrap();
        db::migrate(&mut conn, &db::MIGRATIONS[..1], &config.backup_dir()).unwrap();
        assert_eq!(db::schema_version(&conn).unwrap(), 1);
    }
    let core = Core::open(config.clone()).unwrap();
    core.with_db(|o| {
        assert_eq!(db::schema_version(&o.conn)?, db::MIGRATIONS.len() as i64);
        let n: i64 =
            o.conn
                .query_row("SELECT count(*) FROM audit_log WHERE action = 'db.migrate'", [], |r| r.get(0))?;
        assert_eq!(n, 1);
        Ok(())
    })
    .unwrap();
    let backups: Vec<_> =
        std::fs::read_dir(config.backup_dir()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert!(backups.iter().any(|f| f.to_string_lossy().starts_with("pre-migration-v1-to-v")), "{backups:?}");
}

impl T {
    /// The full error (code + field + rule) of a call that must fail.
    fn err(&self, method: &str, params: Value, token: Option<&str>) -> artaveo_shared::RpcError {
        match self.core.handle(RpcRequest { method: method.into(), params, token: token.map(Into::into) }) {
            RpcResponse::Ok { .. } => panic!("{method} unexpectedly succeeded"),
            RpcResponse::Error { error } => error,
        }
    }
}

fn field_rule(e: &artaveo_shared::RpcError) -> (Option<&str>, Option<String>) {
    (e.field.as_deref(), e.rule.map(|r| serde_json::to_value(r).unwrap().as_str().unwrap().to_string()))
}

/// OF-002: every validation error names the exact request field and rule,
/// so the UI can show it under that input instead of "invalid data".
#[test]
fn validation_errors_name_the_field_and_rule() {
    let t = T::new();
    let base = json!({"clinic_name": "C", "owner_username": "owner", "owner_display_name": "O",
                      "owner_password": "owner-pass-1", "language": "fa"});
    let with = |k: &str, v: Value| {
        let mut p = base.clone();
        p[k] = v;
        p
    };
    let e = t.err(m::APP_SETUP, with("owner_username", json!("احمد")), None);
    assert_eq!(e.code, ErrorCode::Validation);
    assert_eq!(field_rule(&e), (Some("owner_username"), Some("username_format".into())));
    let e = t.err(m::APP_SETUP, with("owner_password", json!("short")), None);
    assert_eq!(field_rule(&e), (Some("owner_password"), Some("password_too_short".into())));
    let e = t.err(m::APP_SETUP, with("clinic_name", json!("  ")), None);
    assert_eq!(field_rule(&e), (Some("clinic_name"), Some("clinic_name_length".into())));
    let e = t.err(m::APP_SETUP, with("owner_display_name", json!("")), None);
    assert_eq!(field_rule(&e), (Some("owner_display_name"), Some("display_name_length".into())));
    assert_eq!(t.ok(m::APP_STATUS, json!({}), None)["state"], "needs_setup", "nothing created");

    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let user =
        |u: &str, p: &str| json!({"username": u, "display_name": "D", "password": p, "role": "doctor"});
    let e = t.err(m::USERS_CREATE, user("احمد", "doctor-pass-1"), Some(&owner));
    assert_eq!(field_rule(&e), (Some("username"), Some("username_format".into())));
    let e = t.err(m::USERS_CREATE, user("doctor", "1234567"), Some(&owner));
    assert_eq!(field_rule(&e), (Some("password"), Some("password_too_short".into())));
    let e = t.err(m::USERS_CREATE, user("Owner", "doctor-pass-1"), Some(&owner));
    assert_eq!(field_rule(&e), (Some("username"), Some("username_taken".into())));

    let mut s = t.ok(m::SETTINGS_GET, json!({}), Some(&owner));
    s["daily_backup_hour"] = json!(24);
    let e = t.err(m::SETTINGS_UPDATE, s, Some(&owner));
    assert_eq!(field_rule(&e), (Some("daily_backup_hour"), Some("backup_hour_range".into())));

    let e = t.err(
        m::AUTH_CHANGE_PASSWORD,
        json!({"current_password": "wrong-pass", "new_password": "new-pass-123"}),
        Some(&owner),
    );
    assert_eq!(e.code, ErrorCode::InvalidCredentials);
    assert_eq!(field_rule(&e), (Some("current_password"), Some("wrong_password".into())));
    let e = t.err(
        m::AUTH_CHANGE_PASSWORD,
        json!({"current_password": "owner-pass-1", "new_password": "short"}),
        Some(&owner),
    );
    assert_eq!(field_rule(&e), (Some("new_password"), Some("password_too_short".into())));

    let mut c = t.ok(m::CLINIC_GET, json!({}), Some(&owner));
    c["working_hours"] = json!([{"day": 0, "closed": false, "open": "16:00", "close": "08:00"}]);
    let e = t.err(m::CLINIC_UPDATE, c, Some(&owner));
    assert_eq!(field_rule(&e), (Some("working_hours"), Some("working_hours".into())));

    t.ok(m::SESSION_LOCK, json!({}), Some(&owner));
    let e = t.err(m::SESSION_UNLOCK, json!({"password": "nope"}), Some(&owner));
    assert_eq!(field_rule(&e), (Some("password"), Some("wrong_password".into())));

    // Login stays deliberately vague: no field, so usernames cannot be probed.
    let e = t.err(m::AUTH_LOGIN, json!({"username": "owner", "password": "nope"}), None);
    assert_eq!(field_rule(&e), (None, None));
}

/// OF-008: activity reported by the UI (`session.touch`) keeps the session
/// unlocked; after an unlock, the very next request succeeds.
#[test]
fn ui_activity_heartbeat_prevents_a_spurious_lock() {
    let t = T::new();
    t.setup();
    let tok = t.login("owner", "owner-pass-1");
    let mut s = t.ok(m::SETTINGS_GET, json!({}), Some(&tok));
    s["session_timeout_minutes"] = json!(1);
    t.ok(m::SETTINGS_UPDATE, s, Some(&tok));

    // A user who keeps working (mouse/keyboard → heartbeat every few seconds)
    // but sends no other request is never locked, however long they work.
    for _ in 0..5 {
        t.core.lock_sessions().age(&tok, Duration::from_secs(50));
        let st = t.ok(m::SESSION_TOUCH, json!({}), Some(&tok));
        assert_eq!(st["locked"], false);
        assert!(st["idle_seconds_left"].as_u64().unwrap() >= 59);
    }
    // `session.state` (the UI's poll) is not activity.
    t.core.lock_sessions().age(&tok, Duration::from_secs(50));
    t.ok(m::SESSION_STATE, json!({}), Some(&tok));
    t.core.lock_sessions().age(&tok, Duration::from_secs(15));
    assert_eq!(t.ok(m::SESSION_STATE, json!({}), Some(&tok))["locked"], true);

    // Requests that were in flight while locked fail with session_locked …
    assert_eq!(t.call(m::SYSTEM_INFO, json!({}), Some(&tok)), Err(ErrorCode::SessionLocked));
    // … but after the correct password everything works at once, repeatedly.
    let st = t.ok(m::SESSION_UNLOCK, json!({"password": "owner-pass-1"}), Some(&tok));
    assert_eq!(st["locked"], false);
    for _ in 0..3 {
        t.ok(m::SYSTEM_INFO, json!({}), Some(&tok));
        assert_eq!(t.ok(m::SESSION_STATE, json!({}), Some(&tok))["locked"], false);
    }
}

/// Roadmap 2.1b / OF-011: the clinic logo is shown in the header and on the
/// login screen (public), and can be replaced later from Clinic Info.
#[test]
fn clinic_logo_is_public_replaceable_and_permissioned() {
    let t = T::new();
    assert_eq!(t.ok(m::APP_CLINIC_LOGO, json!({}), None)["data_url"], Value::Null);
    t.setup();
    assert_eq!(t.ok(m::APP_CLINIC_LOGO, json!({}), None)["data_url"], Value::Null);
    let owner = t.login("owner", "owner-pass-1");
    let png = data_encoding::BASE64.encode(b"\x89PNG\r\n\x1a\nfake");
    let r = t.ok(m::CLINIC_SET_LOGO, json!({"logo_base64": png, "logo_file_name": "Logo.PNG"}), Some(&owner));
    let url = r["data_url"].as_str().unwrap().to_string();
    assert!(url.starts_with("data:image/png;base64,"), "{url}");
    assert_eq!(t.ok(m::APP_CLINIC_LOGO, json!({}), None)["data_url"], json!(url));

    let e =
        t.err(m::CLINIC_SET_LOGO, json!({"logo_base64": png, "logo_file_name": "logo.gif"}), Some(&owner));
    assert_eq!(field_rule(&e), (Some("logo"), Some("logo_type".into())));

    t.ok(
        m::USERS_CREATE,
        json!({"username": "reception", "display_name": "پذیرش", "password": "recep-pass-1", "role": "receptionist"}),
        Some(&owner),
    );
    let rec = t.login("reception", "recep-pass-1");
    assert_eq!(
        t.call(m::CLINIC_SET_LOGO, json!({"logo_base64": null}), Some(&rec)),
        Err(ErrorCode::Forbidden)
    );

    assert_eq!(t.ok(m::CLINIC_SET_LOGO, json!({"logo_base64": null}), Some(&owner))["data_url"], Value::Null);
    assert_eq!(t.ok(m::APP_CLINIC_LOGO, json!({}), None)["data_url"], Value::Null);
    assert!(t.audit_actions(&owner).contains(&"clinic.set_logo".to_string()));
}

/// OF-012: the idle lock follows real user input only. Background requests
/// (About refreshes every 5 s) must not keep an unattended screen unlocked,
/// a new timeout applies at once, and `idle_ms` makes the timer exact.
#[test]
fn idle_lock_is_exact_and_ignores_background_requests() {
    let t = T::new();
    t.setup();
    let tok = t.login("owner", "owner-pass-1");
    let mut s = t.ok(m::SETTINGS_GET, json!({}), Some(&tok));
    s["session_timeout_minutes"] = json!(1);
    t.ok(m::SETTINGS_UPDATE, s, Some(&tok)); // applies to the running session at once

    // Polls every few seconds for 59 s: still unlocked, but they do not count as activity.
    for _ in 0..11 {
        t.core.lock_sessions().age(&tok, Duration::from_secs(5));
        t.ok(m::SYSTEM_INFO, json!({}), Some(&tok));
    }
    t.core.lock_sessions().age(&tok, Duration::from_secs(4));
    let st = t.ok(m::SESSION_STATE, json!({}), Some(&tok));
    assert_eq!(st["locked"], false, "59 s idle with a 60 s timeout");
    assert!(st["idle_seconds_left"].as_u64().unwrap() <= 1);
    t.core.lock_sessions().age(&tok, Duration::from_secs(1));
    assert_eq!(t.ok(m::SESSION_STATE, json!({}), Some(&tok))["locked"], true, "locks at 60 s");

    // `idle_ms`: a heartbeat sent 4 s after the last mouse move counts from the move.
    t.ok(m::SESSION_UNLOCK, json!({"password": "owner-pass-1"}), Some(&tok));
    t.core.lock_sessions().age(&tok, Duration::from_secs(30));
    let st = t.ok(m::SESSION_TOUCH, json!({"idle_ms": 4000}), Some(&tok));
    let left = st["idle_seconds_left"].as_u64().unwrap();
    assert!((54..=56).contains(&left), "{left}");
    // A heartbeat never moves the timer backwards.
    let st = t.ok(m::SESSION_TOUCH, json!({"idle_ms": 50_000}), Some(&tok));
    assert!(st["idle_seconds_left"].as_u64().unwrap() >= 54);
}

// ───────────────────────────── patients (Phase 3) ─────────────────────────────

#[test]
fn patient_crud_search_and_duplicate_detection() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");

    assert_eq!(
        t.call(m::PATIENTS_CREATE, json!({"full_name": ""}), Some(&owner)),
        Err(ErrorCode::Validation)
    );

    let p1 = t.ok(
        m::PATIENTS_CREATE,
        json!({"full_name": "احمد خان", "father_name": "کریم", "phone": "0700123456"}),
        Some(&owner),
    );
    assert_eq!(p1["patient_number"], "P-000001");
    assert_eq!(p1["status"], "active");

    let p2 = t.ok(m::PATIENTS_CREATE, json!({"full_name": "Zarghuna"}), Some(&owner));
    assert_eq!(p2["patient_number"], "P-000002");

    // Search finds by partial name, patient number and phone, ignoring letter/digit-script variants.
    let found = t.ok(m::PATIENTS_LIST, json!({"query": "احمد"}), Some(&owner));
    assert_eq!(found["total"], 1);
    assert_eq!(found["items"][0]["id"], p1["id"]);
    let found = t.ok(m::PATIENTS_LIST, json!({"query": "علي"}), Some(&owner)); // Arabic yeh, no match expected
    assert_eq!(found["total"], 0);
    let found = t.ok(m::PATIENTS_LIST, json!({"query": "P-000002"}), Some(&owner));
    assert_eq!(found["items"][0]["id"], p2["id"]);
    let found = t.ok(m::PATIENTS_LIST, json!({"query": "0700123456"}), Some(&owner));
    assert_eq!(found["items"][0]["id"], p1["id"]);
    let all = t.ok(m::PATIENTS_LIST, json!({}), Some(&owner));
    assert_eq!(all["total"], 2);

    // Duplicate detection: same name or same phone.
    let dups =
        t.ok(m::PATIENTS_CHECK_DUPLICATE, json!({"full_name": "احمد خان", "phone": null}), Some(&owner));
    assert_eq!(dups.as_array().unwrap().len(), 1);
    let dups = t.ok(
        m::PATIENTS_CHECK_DUPLICATE,
        json!({"full_name": "someone else", "phone": "0700123456"}),
        Some(&owner),
    );
    assert_eq!(dups.as_array().unwrap().len(), 1);

    // Update with optimistic locking.
    let mut update = p1.clone();
    update["full_name"] = json!("احمد خان 2");
    update["status"] = json!("active");
    let updated = t.ok(m::PATIENTS_UPDATE, update.clone(), Some(&owner));
    assert_eq!(updated["full_name"], "احمد خان 2");
    assert_eq!(updated["version"], 2);
    assert_eq!(t.call(m::PATIENTS_UPDATE, update, Some(&owner)), Err(ErrorCode::Conflict), "stale version");

    // Soft delete: disappears from the active list, search no longer finds it.
    t.ok(m::PATIENTS_DELETE, json!({"id": p2["id"], "version": p2["version"]}), Some(&owner));
    let all = t.ok(m::PATIENTS_LIST, json!({}), Some(&owner));
    assert_eq!(all["total"], 1);
    assert_eq!(
        t.call(m::PATIENTS_GET, json!({"patient_id": p2["id"]}), Some(&owner)),
        Err(ErrorCode::NotFound)
    );

    let actions = t.audit_actions(&owner);
    assert!(actions.contains(&"patient.create".to_string()));
    assert!(actions.contains(&"patient.update".to_string()));
    assert!(actions.contains(&"patient.delete".to_string()));
}

#[test]
fn patient_merge_folds_the_losing_record() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let keep =
        t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad Khan", "phone": "0700000001"}), Some(&owner));
    assert_eq!(
        t.call(m::PATIENTS_CREATE, json!({"full_name": "Ahmad Khan", "phone": "0700000002"}), Some(&owner)),
        Err(ErrorCode::Validation),
        "same name is flagged as a possible duplicate"
    );
    let dupe = t.ok(
        m::PATIENTS_CREATE,
        json!({"full_name": "Ahmad Khan", "phone": "0700000002", "allow_duplicate": true}),
        Some(&owner),
    );

    assert_eq!(
        t.call(
            m::PATIENTS_MERGE,
            json!({"keep_id": keep["id"], "merge_id": keep["id"], "merge_id_version": 1}),
            Some(&owner)
        ),
        Err(ErrorCode::Validation)
    );

    let kept = t.ok(
        m::PATIENTS_MERGE,
        json!({"keep_id": keep["id"], "merge_id": dupe["id"], "merge_id_version": dupe["version"]}),
        Some(&owner),
    );
    assert_eq!(kept["id"], keep["id"]);
    let merged = t.ok(m::PATIENTS_GET, json!({"patient_id": dupe["id"]}), Some(&owner));
    assert_eq!(merged["status"], "inactive");
    assert_eq!(merged["merged_into_id"], keep["id"]);
    let dups =
        t.ok(m::PATIENTS_CHECK_DUPLICATE, json!({"full_name": "Ahmad Khan", "phone": null}), Some(&owner));
    assert_eq!(dups.as_array().unwrap().len(), 1);
}

/// The id of the seeded checklist question `code` (M1).
fn question(t: &T, token: &str, code: &str) -> Value {
    t.ok(m::MEDICAL_QUESTIONS_LIST, json!({"include_inactive": true}), Some(token))
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["code"] == code)
        .unwrap_or_else(|| panic!("question {code}"))
        .clone()
}

#[test]
fn medical_history_checklist_answers_alerts_and_optimistic_locking() {
    // M1 / OF-025: a yes/no/unknown checklist replaces the free-text fields.
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let patient = t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad Khan"}), Some(&owner));

    let questions = t.ok(m::MEDICAL_QUESTIONS_LIST, json!({}), Some(&owner));
    let questions = questions.as_array().unwrap();
    assert_eq!(questions.len(), 34);
    assert_eq!(questions[0]["group_code"], "cardio", "checklist order: groups as in the spec");
    let anticoag = question(&t, &owner, "anticoagulants");
    assert_eq!(anticoag["alert"], true);
    assert_eq!(anticoag["detail_kind"], "text");
    assert_eq!(anticoag["label"]["fa"], "مصرف رقیق‌کننده خون (وارفارین، آسپرین، کلوپیدوگرل و…)");
    assert!(anticoag["alert_note"]["en"].as_str().unwrap().contains("prescribing doctor"));
    let diabetes = question(&t, &owner, "diabetes");
    assert_eq!(diabetes["choices"], json!(["controlled", "uncontrolled"]));
    let pregnant = question(&t, &owner, "pregnant");
    assert_eq!(
        (pregnant["female_only"].as_bool(), pregnant["detail_kind"].as_str()),
        (Some(true), Some("months"))
    );
    let hiv = question(&t, &owner, "hiv");

    let empty = t.ok(m::MEDICAL_HISTORY_GET, json!({"patient_id": patient["id"]}), Some(&owner));
    assert_eq!((empty["version"].as_i64(), empty["review_due"].as_bool()), (Some(0), Some(true)));
    assert_eq!(empty["answers"], json!([]));

    let save = |version: i64, answers: Value| {
        t.call(
            m::MEDICAL_HISTORY_UPDATE,
            json!({"patient_id": patient["id"], "version": version, "answers": answers, "notes": " دندان قروچه "}),
            Some(&owner),
        )
    };
    let saved = save(
        0,
        json!([
            {"question_id": anticoag["id"], "answer": "yes", "detail_text": " Warfarin "},
            {"question_id": diabetes["id"], "answer": "yes", "detail_choice": "uncontrolled"},
            // A detail only belongs to a "yes": it is dropped otherwise.
            {"question_id": hiv["id"], "answer": "no", "detail_text": "ignored"},
            {"question_id": pregnant["id"], "answer": "yes", "detail_text": "۵"}
        ]),
    )
    .unwrap();
    assert_eq!(saved["version"], 1);
    assert_eq!(saved["review_due"], false);
    assert_eq!(saved["reviewed_by_name"], "مالک");
    assert_eq!(saved["notes"], "دندان قروچه");
    let answers = saved["answers"].as_array().unwrap();
    assert_eq!(answers.len(), 4);
    assert_eq!(answers[0]["question_id"], anticoag["id"], "answers in checklist order");
    assert_eq!(answers[0]["detail_text"], "Warfarin");
    let hiv_answer = answers.iter().find(|a| a["question_id"] == hiv["id"]).unwrap();
    assert_eq!((hiv_answer["answer"].as_str(), hiv_answer["detail_text"].as_str()), (Some("no"), None));
    assert_eq!(answers.iter().find(|a| a["question_id"] == pregnant["id"]).unwrap()["detail_text"], "5");

    // Each rule names the answer at fault (OF-002).
    let field_of = |answers: Value| {
        let (_, field, rule) = rule_of(
            &t,
            m::MEDICAL_HISTORY_UPDATE,
            json!({"patient_id": patient["id"], "version": 1, "answers": answers}),
            &owner,
        );
        (field, rule)
    };
    let pregnant_field = Some(format!("answers.{}", pregnant["id"].as_str().unwrap()));
    assert_eq!(
        field_of(json!([{"question_id": pregnant["id"], "answer": "yes", "detail_text": "12"}])),
        (pregnant_field.clone(), Some("medical_months_range".into()))
    );
    assert_eq!(
        field_of(json!([{"question_id": diabetes["id"], "answer": "yes", "detail_choice": "sometimes"}])).1,
        Some("medical_choice".into())
    );
    assert_eq!(
        field_of(json!([{"question_id": "nope", "answer": "yes"}])).1,
        Some("medical_question_not_found".into())
    );
    let long = "x".repeat(301);
    assert_eq!(
        field_of(json!([{"question_id": anticoag["id"], "answer": "yes", "detail_text": long}])).1,
        Some("medical_detail_length".into())
    );

    // A stale view is refused; the full set replaces the old one.
    assert_eq!(save(0, json!([])), Err(ErrorCode::Conflict));
    let replaced = save(1, json!([{"question_id": hiv["id"], "answer": "unknown"}])).unwrap();
    assert_eq!(replaced["answers"].as_array().unwrap().len(), 1);

    // "Reviewed, nothing changed" only moves the review date (and the version).
    let reviewed =
        t.ok(m::MEDICAL_HISTORY_REVIEW, json!({"patient_id": patient["id"], "version": 2}), Some(&owner));
    assert_eq!((reviewed["version"].as_i64(), reviewed["answers"].as_array().unwrap().len()), (Some(3), 1));
    assert_eq!(
        t.call(m::MEDICAL_HISTORY_REVIEW, json!({"patient_id": patient["id"], "version": 2}), Some(&owner)),
        Err(ErrorCode::Conflict)
    );

    let actions = t.audit_actions(&owner);
    assert_eq!(actions.iter().filter(|a| a.as_str() == "patient.medical_history_update").count(), 2);
    assert!(actions.contains(&"patient.medical_history_review".to_string()));
}

#[test]
fn clinic_adds_its_own_questions_and_can_switch_system_ones_off() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let q = t.ok(
        m::MEDICAL_QUESTIONS_CREATE,
        json!({"group_code": "other", "label": {"fa": "سابقه کرونا", "ps": "", "en": ""}, "detail_kind": "text", "alert": true}),
        Some(&owner),
    );
    assert_eq!((q["is_system"].as_bool(), q["label"]["en"].as_str()), (Some(false), Some("سابقه کرونا")));
    assert_eq!(q["detail_label"]["fa"], "توضیح");
    let (_, field, rule) = rule_of(
        &t,
        m::MEDICAL_QUESTIONS_CREATE,
        json!({"group_code": "other", "label": {"fa": " ", "ps": "", "en": ""}}),
        &owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("label"), Some("medical_question_label")));
    let (_, field, rule) = rule_of(
        &t,
        m::MEDICAL_QUESTIONS_CREATE,
        json!({"group_code": "other", "label": {"fa": "x", "ps": "x", "en": "x"}, "detail_kind": "months"}),
        &owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("detail_kind"), Some("medical_detail_kind")));

    // A system question can be switched off, never reworded.
    let mut thyroid = question(&t, &owner, "thyroid");
    thyroid["is_active"] = json!(false);
    let off = t.ok(m::MEDICAL_QUESTIONS_UPDATE, thyroid.clone(), Some(&owner));
    assert_eq!(off["is_active"], false);
    assert_eq!(t.ok(m::MEDICAL_QUESTIONS_LIST, json!({}), Some(&owner)).as_array().unwrap().len(), 34);
    let mut reworded = off.clone();
    reworded["label"]["fa"] = json!("چیز دیگر");
    let (_, _, rule) = rule_of(&t, m::MEDICAL_QUESTIONS_UPDATE, reworded, &owner);
    assert_eq!(rule.as_deref(), Some("medical_system_question"));
    // A start-up (seed sync) keeps the clinic's choice.
    let t = t.reopen();
    let owner = t.login("owner", "owner-pass-1");
    assert_eq!(question(&t, &owner, "thyroid")["is_active"], false);

    // Reception (no settings.manage) can read the questions but not change them.
    let mut reception_cannot = q.clone();
    reception_cannot["alert"] = json!(false);
    t.ok(
        m::USERS_CREATE,
        json!({"username": "rec", "display_name": "Reception", "password": "reception-1", "role": "receptionist"}),
        Some(&owner),
    );
    let rec = t.login("rec", "reception-1");
    t.ok(m::MEDICAL_QUESTIONS_LIST, json!({}), Some(&rec));
    assert_eq!(t.call(m::MEDICAL_QUESTIONS_UPDATE, reception_cannot, Some(&rec)), Err(ErrorCode::Forbidden));
}

#[test]
fn migration_moves_old_free_text_medical_history_into_the_notes() {
    // OF-025: a history typed as free text before v0.5.0 survives the move to the checklist.
    let dir = tempfile::tempdir().unwrap();
    let config = Config::new(Environment::Test, dir.path().to_path_buf());
    let recovery = RecoveryKey::generate();
    let protector = default_protector();
    let path = config.db_path();
    {
        let (mut conn, _key) = db::create(&path, protector.as_ref(), &recovery).unwrap();
        let v9: Vec<db::Migration> = db::MIGRATIONS
            .iter()
            .filter(|m| m.version <= 9)
            .map(|m| db::Migration { version: m.version, name: m.name, sql: m.sql })
            .collect();
        db::migrate(&mut conn, &v9, &config.backup_dir()).unwrap();
        conn.execute_batch(
            "INSERT INTO db_meta(key, value) VALUES ('clinic_name', 'Old'), ('default_language', 'fa'), ('clinic_id', 'x'),
                    ('created_at', '2026-01-01T00:00:00.000Z'), ('created_with_version', '0.4.1');
             INSERT INTO patient(id, patient_number, full_name, registration_date, created_at, updated_at)
             VALUES ('p1', 'P-000001', 'Old Patient', '2026-01-01', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');
             INSERT INTO patient_medical_history(id, patient_id, allergies, chronic_conditions, notes, created_at, updated_at)
             VALUES ('h1', 'p1', 'Penicillin', ' دیابت ', 'یادداشت قدیمی', '2026-01-01T00:00:00.000Z', '2026-02-01T00:00:00.000Z');",
        )
        .unwrap();
    }
    let core = Core::open(config).unwrap();
    let h = core.with_db(|o| crate::medical::get_history(&o.conn, "p1")).unwrap();
    assert_eq!(h.notes.as_deref(), Some("حساسیت‌ها: Penicillin\nبیماری‌های مزمن: دیابت\nیادداشت قدیمی"));
    assert_eq!(h.reviewed_at.as_deref(), Some("2026-02-01T00:00:00.000Z"));
    assert!(h.review_due, "reviewed more than six months ago");
    assert_eq!(h.version, 1);
    assert!(h.answers.is_empty());
}

#[test]
fn attachments_are_stored_encrypted_and_scoped_to_their_patient() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let patient = t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad Khan"}), Some(&owner));

    let data_base64 = data_encoding::BASE64.encode(b"fake x-ray bytes");
    let a = t.ok(
        m::ATTACHMENTS_UPLOAD,
        json!({"patient_id": patient["id"], "kind": "xray", "file_name": "molar.png", "data_base64": data_base64, "tooth": "26"}),
        Some(&owner),
    );
    assert_eq!(a["kind"], "xray");
    assert_eq!(a["mime_type"], "image/png");

    let list = t.ok(m::ATTACHMENTS_LIST, json!({"patient_id": patient["id"]}), Some(&owner));
    assert_eq!(list.as_array().unwrap().len(), 1);

    let file = t.ok(m::ATTACHMENTS_FILE, json!({"id": a["id"]}), Some(&owner));
    let url = file["data_url"].as_str().unwrap();
    assert!(url.starts_with("data:image/png;base64,"));

    let dir = t.core.config().attachments_dir();
    let on_disk: Vec<u8> = walk_first_file(&dir);
    let needle = b"fake x-ray";
    assert!(!on_disk.windows(needle.len()).any(|w| w == needle));

    t.ok(m::ATTACHMENTS_DELETE, json!({"id": a["id"], "version": a["version"]}), Some(&owner));
    let list = t.ok(m::ATTACHMENTS_LIST, json!({"patient_id": patient["id"]}), Some(&owner));
    assert_eq!(list.as_array().unwrap().len(), 0);
}

fn walk_first_file(dir: &std::path::Path) -> Vec<u8> {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            if let Some(f) = std::fs::read_dir(entry.path()).unwrap().next() {
                return std::fs::read(f.unwrap().path()).unwrap();
            }
        }
    }
    panic!("no attachment file found under {dir:?}");
}

#[test]
fn import_previews_then_commits_only_valid_rows() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let csv = "full_name,father_name,phone,secondary_phone,date_of_birth,address\nAhmad Khan,Karim,0700123456,,1990-01-01,Kabul\n,Bad Row,123,,,\nZarghuna,,,,,";
    let csv_base64 = data_encoding::BASE64.encode(csv.as_bytes());

    let preview = t.ok(
        m::PATIENTS_IMPORT,
        json!({"file_base64": csv_base64, "file_name": "p.csv", "commit": false}),
        Some(&owner),
    );
    assert_eq!(preview["total"], 3);
    assert_eq!(preview["imported"], 0);
    assert_eq!(preview["skipped"], 1);
    assert_eq!(t.ok(m::PATIENTS_LIST, json!({}), Some(&owner))["total"], 0);

    let result = t.ok(
        m::PATIENTS_IMPORT,
        json!({"file_base64": csv_base64, "file_name": "p.csv", "commit": true}),
        Some(&owner),
    );
    assert_eq!(result["imported"], 2);
    assert_eq!(result["skipped"], 1);
    assert_eq!(t.ok(m::PATIENTS_LIST, json!({}), Some(&owner))["total"], 2);

    // OF-022: the export is a real .xlsx file on disk now, not a CSV string.
    let exported = t.ok(m::PATIENTS_EXPORT, json!({}), Some(&owner));
    assert_eq!(exported["rows"], 2);
    let path = exported["file_path"].as_str().unwrap();
    assert!(path.ends_with(".xlsx") && std::path::Path::new(path).exists());
    let bytes = std::fs::read(path).unwrap();
    let b64 = data_encoding::BASE64.encode(&bytes);
    let info = t.ok(
        m::PATIENTS_IMPORT_INSPECT,
        json!({"file_base64": b64, "file_name": exported["file_name"]}),
        Some(&owner),
    );
    let rows = info["sample_rows"].as_array().unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r[1].as_str().unwrap()).collect();
    assert!(names.contains(&"Ahmad Khan"));
    assert!(names.contains(&"Zarghuna"));
}

#[test]
fn patients_require_permission() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    t.ok(
        m::USERS_CREATE,
        json!({"username": "accountant1", "display_name": "x", "password": "acct-pass-12", "role": "accountant"}),
        Some(&owner),
    );
    let accountant = t.login("accountant1", "acct-pass-12");
    t.ok(m::PATIENTS_LIST, json!({}), Some(&accountant));
    assert_eq!(
        t.call(m::PATIENTS_CREATE, json!({"full_name": "x"}), Some(&accountant)),
        Err(ErrorCode::Forbidden)
    );
}

// ───────────────────────────── Phase 4 ─────────────────────────────

fn rule_of(t: &T, method: &str, params: Value, token: &str) -> (ErrorCode, Option<String>, Option<String>) {
    match t.core.handle(RpcRequest { method: method.into(), params, token: Some(token.into()) }) {
        RpcResponse::Ok { result } => panic!("{method} unexpectedly succeeded: {result}"),
        RpcResponse::Error { error } => (
            error.code,
            error.field,
            error.rule.map(|r| serde_json::to_value(r).unwrap().as_str().unwrap().to_string()),
        ),
    }
}

fn s_of(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_else(|| panic!("{key} missing in {v}")).to_string()
}

struct Clinic4 {
    t: T,
    owner: String,
    doctor: String,
    doctor2: String,
    chair: String,
    chair2: String,
    patient: String,
    patient2: String,
    today: String,
}

fn clinic4() -> Clinic4 {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let d1 = t.ok(
        m::DOCTORS_CREATE,
        json!({"full_name": "دکتر احمد", "specialty": "Orthodontics", "color": "#0E7490"}),
        Some(&owner),
    );
    let d2 = t.ok(m::DOCTORS_CREATE, json!({"full_name": "دکتر زرغونه", "color": "#f59e0b"}), Some(&owner));
    let c1 = t.ok(m::CHAIRS_CREATE, json!({"name": "Chair 1"}), Some(&owner));
    let c2 = t.ok(m::CHAIRS_CREATE, json!({"name": "Chair 2"}), Some(&owner));
    let p1 = t.ok(m::PATIENTS_CREATE, json!({"full_name": "بیمار یکم", "phone": "0700111111"}), Some(&owner));
    let p2 = t.ok(m::PATIENTS_CREATE, json!({"full_name": "بیمار دوم", "phone": "0700222222"}), Some(&owner));
    Clinic4 {
        doctor: s_of(&d1, "id"),
        doctor2: s_of(&d2, "id"),
        chair: s_of(&c1, "id"),
        chair2: s_of(&c2, "id"),
        patient: s_of(&p1, "id"),
        patient2: s_of(&p2, "id"),
        today: crate::clock::today_iso(),
        owner,
        t,
    }
}

impl Clinic4 {
    fn book(
        &self,
        patient: &str,
        doctor: &str,
        chair: Option<&str>,
        date: &str,
        start: &str,
        end: &str,
    ) -> Result<Value, ErrorCode> {
        self.t.call(
            m::APPOINTMENTS_CREATE,
            json!({"patient_id": patient, "doctor_id": doctor, "chair_id": chair, "date": date, "start_time": start, "end_time": end, "reason": "check"}),
            Some(&self.owner),
        )
    }

    fn status(&self, appt: &Value, status: &str) -> Result<Value, ErrorCode> {
        self.t.call(
            m::APPOINTMENTS_SET_STATUS,
            json!({"id": appt["id"], "version": appt["version"], "status": status}),
            Some(&self.owner),
        )
    }

    fn get(&self, id: &Value) -> Value {
        self.t.ok(m::APPOINTMENTS_GET, json!({"id": id}), Some(&self.owner))
    }
}

/// Day offset helper: the ISO date `n` days after `date`.
fn plus_days(date: &str, n: i64) -> String {
    let d = crate::scheduling::parse_date("d", date).unwrap() + time::Duration::days(n);
    crate::scheduling::format_date(d)
}

#[test]
fn doctors_and_chairs_have_profiles_validation_and_permissions() {
    let c = clinic4();
    let t = &c.t;
    let doctors = t.ok(m::DOCTORS_LIST, json!({}), Some(&c.owner));
    assert_eq!(doctors.as_array().unwrap().len(), 2);
    assert_eq!(doctors[0]["color"], "#0e7490", "colour is stored lower-case");
    assert_eq!(doctors[0]["specialty"], "Orthodontics");

    let (code, field, rule) =
        rule_of(t, m::DOCTORS_CREATE, json!({"full_name": "x", "color": "red"}), &c.owner);
    assert_eq!(
        (code, field.as_deref(), rule.as_deref()),
        (ErrorCode::Validation, Some("color"), Some("color_format"))
    );
    let (_, field, rule) =
        rule_of(t, m::DOCTORS_CREATE, json!({"full_name": " ", "color": "#000000"}), &c.owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("full_name"), Some("full_name_length")));
    let (_, field, rule) = rule_of(t, m::CHAIRS_CREATE, json!({"name": "chair 1"}), &c.owner);
    assert_eq!(
        (field.as_deref(), rule.as_deref()),
        (Some("name"), Some("chair_name_taken")),
        "names are unique, case-insensitively"
    );

    // A doctor may be linked to a login, but one login serves one doctor.
    let user = t.ok(
        m::USERS_CREATE,
        json!({"username": "dr.ahmad", "display_name": "د", "password": "doctor-pass-1", "role": "doctor"}),
        Some(&c.owner),
    );
    let linked = t.ok(
        m::DOCTORS_UPDATE,
        json!({"id": c.doctor, "version": doctors[0]["version"], "full_name": "دکتر احمد", "color": "#0e7490", "status": "active", "user_id": user["id"]}),
        Some(&c.owner),
    );
    assert_eq!(linked["username"], "dr.ahmad");
    let (_, field, rule) = rule_of(
        t,
        m::DOCTORS_UPDATE,
        json!({"id": c.doctor2, "version": doctors[1]["version"], "full_name": "x", "color": "#000000", "status": "active", "user_id": user["id"]}),
        &c.owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("user_id"), Some("user_already_doctor")));

    // Inactive doctors disappear from the default list but stay available for history.
    t.ok(
        m::DOCTORS_UPDATE,
        json!({"id": c.doctor2, "version": doctors[1]["version"], "full_name": "دکتر زرغونه", "color": "#f59e0b", "status": "inactive"}),
        Some(&c.owner),
    );
    assert_eq!(t.ok(m::DOCTORS_LIST, json!({}), Some(&c.owner)).as_array().unwrap().len(), 1);
    assert_eq!(
        t.ok(m::DOCTORS_LIST, json!({"include_inactive": true}), Some(&c.owner)).as_array().unwrap().len(),
        2
    );
    let (_, field, rule) = rule_of(
        t,
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient, "doctor_id": c.doctor2, "date": c.today, "start_time": "09:00", "end_time": "09:30"}),
        &c.owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("doctor_id"), Some("doctor_inactive")));

    // A receptionist books patients but cannot manage doctors or chairs; she can still see them.
    t.ok(m::USERS_CREATE, json!({"username": "recep1", "display_name": "r", "password": "recep-pass-12", "role": "receptionist"}), Some(&c.owner));
    let recep = t.login("recep1", "recep-pass-12");
    assert_eq!(t.ok(m::DOCTORS_LIST, json!({}), Some(&recep)).as_array().unwrap().len(), 1);
    assert_eq!(
        t.call(m::DOCTORS_CREATE, json!({"full_name": "x", "color": "#000000"}), Some(&recep)),
        Err(ErrorCode::Forbidden)
    );
    assert_eq!(t.call(m::CHAIRS_CREATE, json!({"name": "x"}), Some(&recep)), Err(ErrorCode::Forbidden));

    let actions = t.audit_actions(&c.owner);
    for a in ["doctor.create", "doctor.update", "chair.create"] {
        assert!(actions.contains(&a.to_string()), "{a} is audited");
    }
}

#[test]
fn schedule_hours_breaks_chairs_and_leave_limit_bookings() {
    let c = clinic4();
    let t = &c.t;
    let doc = t.ok(m::DOCTORS_LIST, json!({}), Some(&c.owner))[0].clone();
    // Find the next Saturday-based day: use the weekday of a fixed future date, 2030-01-05 is a Saturday (day 0).
    let sat = "2030-01-05";
    let sun = "2030-01-06";
    let schedule = t.ok(
        m::DOCTORS_SET_SCHEDULE,
        json!({
            "doctor_id": c.doctor, "version": doc["version"],
            "hours": [{"day": 0, "start": "08:00", "end": "12:00"}, {"day": 0, "start": "14:00", "end": "18:00"}],
            "breaks": [{"day": 0, "start": "10:00", "end": "10:30"}],
            "chair_ids": [c.chair]
        }),
        Some(&c.owner),
    );
    assert_eq!(schedule["hours"].as_array().unwrap().len(), 2);
    assert_eq!(schedule["chair_ids"][0], json!(c.chair));

    assert!(c.book(&c.patient, &c.doctor, Some(&c.chair), sat, "09:00", "09:30").is_ok());
    for (date, start, end, rule) in [
        (sat, "12:30", "13:00", "outside_working_hours"), // lunch gap between the two intervals
        (sat, "07:30", "08:30", "outside_working_hours"),
        (sat, "10:00", "10:30", "doctor_on_break"),
        (sat, "10:15", "10:45", "doctor_on_break"),
        (sun, "09:00", "09:30", "outside_working_hours"), // no hours on Sunday
    ] {
        let (code, _, r) = rule_of(
            t,
            m::APPOINTMENTS_CREATE,
            json!({"patient_id": c.patient2, "doctor_id": c.doctor, "date": date, "start_time": start, "end_time": end}),
            &c.owner,
        );
        assert_eq!((code, r.as_deref()), (ErrorCode::Validation, Some(rule)), "{date} {start}-{end}");
    }
    // Touching the break is fine; so is the second working interval.
    assert!(c.book(&c.patient2, &c.doctor, Some(&c.chair), sat, "10:30", "11:00").is_ok());
    assert!(c.book(&c.patient2, &c.doctor, None, sat, "14:00", "14:30").is_ok());
    // A doctor limited to one chair cannot use another.
    let (_, field, r) = rule_of(
        t,
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient2, "doctor_id": c.doctor, "chair_id": c.chair2, "date": sat, "start_time": "16:00", "end_time": "16:30"}),
        &c.owner,
    );
    assert_eq!((field.as_deref(), r.as_deref()), (Some("chair_id"), Some("chair_not_allowed")));
    // Reception may override the schedule on purpose (an emergency after hours).
    let forced = t.ok(
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient2, "doctor_id": c.doctor, "date": sat, "start_time": "19:00", "end_time": "19:30", "override_schedule": true}),
        Some(&c.owner),
    );
    assert_eq!(forced["start_time"], "19:00");

    // Overlapping intervals in a schedule are rejected with the field named.
    let doc = t.ok(m::DOCTORS_LIST, json!({}), Some(&c.owner))[0].clone();
    let (_, field, r) = rule_of(
        t,
        m::DOCTORS_SET_SCHEDULE,
        json!({"doctor_id": c.doctor, "version": doc["version"], "hours": [{"day": 0, "start": "08:00", "end": "12:00"}, {"day": 0, "start": "11:00", "end": "13:00"}]}),
        &c.owner,
    );
    assert_eq!((field.as_deref(), r.as_deref()), (Some("hours"), Some("schedule_overlap")));

    // Leave blocks the day — and tells how many existing visits are affected.
    let leave = t.ok(
        m::DOCTORS_ADD_LEAVE,
        json!({"doctor_id": c.doctor, "start_date": sat, "end_date": plus_days(sat, 2), "reason": "Hajj"}),
        Some(&c.owner),
    );
    assert_eq!(leave["affected_appointments"], 4);
    let (_, _, r) = rule_of(
        t,
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient, "doctor_id": c.doctor, "date": sat, "start_time": "15:00", "end_time": "15:30"}),
        &c.owner,
    );
    assert_eq!(r.as_deref(), Some("doctor_on_leave"));
    let (_, field, r) = rule_of(
        t,
        m::DOCTORS_ADD_LEAVE,
        json!({"doctor_id": c.doctor, "start_date": sat, "end_date": "2029-01-01"}),
        &c.owner,
    );
    assert_eq!((field.as_deref(), r.as_deref()), (Some("end_date"), Some("leave_range")));
    // Removing the leave frees the day again.
    let leaves = t.ok(m::DOCTORS_LIST, json!({}), Some(&c.owner))[0]["leaves"].clone();
    t.ok(
        m::DOCTORS_DELETE_LEAVE,
        json!({"id": leaves[0]["id"], "version": leaves[0]["version"]}),
        Some(&c.owner),
    );
    assert!(c.book(&c.patient, &c.doctor, None, sat, "15:00", "15:30").is_ok());

    // A doctor with no weekly hours at all has no hour restriction.
    assert!(c.book(&c.patient, &c.doctor2, None, sun, "03:00", "03:30").is_ok());
}

#[test]
fn double_booking_is_refused_for_doctor_chair_and_patient() {
    let c = clinic4();
    let day = plus_days(&c.today, 3);
    let first = c.book(&c.patient, &c.doctor, Some(&c.chair), &day, "09:00", "09:30").unwrap();
    assert_eq!(first["status"], "scheduled");
    assert_eq!(first["start_at"].as_str().unwrap().len(), 24);

    let attempt = |patient: &str, doctor: &str, chair: Option<&str>, start: &str, end: &str| {
        let (code, field, rule) = rule_of(
            &c.t,
            m::APPOINTMENTS_CREATE,
            json!({"patient_id": patient, "doctor_id": doctor, "chair_id": chair, "date": day, "start_time": start, "end_time": end}),
            &c.owner,
        );
        assert_eq!(code, ErrorCode::Validation);
        (field.unwrap(), rule.unwrap())
    };
    // Same doctor, overlapping by one minute.
    assert_eq!(
        attempt(&c.patient2, &c.doctor, None, "09:29", "10:00"),
        ("doctor_id".into(), "doctor_busy".into())
    );
    // Different doctor, same chair.
    assert_eq!(
        attempt(&c.patient2, &c.doctor2, Some(&c.chair), "09:15", "09:45"),
        ("chair_id".into(), "chair_busy".into())
    );
    // Same patient with another doctor at the same time.
    assert_eq!(
        attempt(&c.patient, &c.doctor2, Some(&c.chair2), "09:00", "09:20"),
        ("patient_id".into(), "patient_busy".into())
    );
    // Back-to-back and different resources are fine.
    assert!(c.book(&c.patient2, &c.doctor, Some(&c.chair), &day, "09:30", "10:00").is_ok());
    assert!(c.book(&c.patient2, &c.doctor2, Some(&c.chair2), &day, "09:00", "09:30").is_ok());

    // A cancelled appointment frees its slot for everyone.
    c.status(&first, "cancelled").unwrap();
    assert!(
        c.book(&c.patient, &c.doctor, Some(&c.chair2), &day, "09:00", "09:15").is_err(),
        "chair 2 is busy with the other doctor"
    );
    assert!(c.book(&c.patient, &c.doctor, Some(&c.chair), &day, "09:00", "09:30").is_ok());

    // Moving an appointment onto a busy slot fails; onto itself (a notes edit) does not.
    let a = c.book(&c.patient2, &c.doctor, None, &day, "11:00", "11:30").unwrap();
    let blocked = c.t.call(
        m::APPOINTMENTS_UPDATE,
        json!({"id": a["id"], "version": a["version"], "doctor_id": c.doctor, "date": day, "start_time": "09:15", "end_time": "09:45"}),
        Some(&c.owner),
    );
    assert_eq!(blocked, Err(ErrorCode::Validation));
    // `chair_id` is resent explicitly throughout, as the real UI always does (OF-035): omitting it
    // means "clear the chair", not "leave it alone".
    let edited = c.t.ok(
        m::APPOINTMENTS_UPDATE,
        json!({"id": a["id"], "version": a["version"], "doctor_id": c.doctor, "chair_id": a["chair_id"], "date": day, "start_time": "11:00", "end_time": "11:30", "notes": "bring X-rays"}),
        Some(&c.owner),
    );
    assert_eq!(edited["notes"], "bring X-rays");
    assert_eq!(edited["chair_id"], a["chair_id"]);
    // Drag & drop is the same call with a new time; the old slot becomes free.
    let moved = c.t.ok(
        m::APPOINTMENTS_UPDATE,
        json!({"id": a["id"], "version": edited["version"], "doctor_id": c.doctor, "chair_id": edited["chair_id"], "date": day, "start_time": "13:00", "end_time": "13:30"}),
        Some(&c.owner),
    );
    assert_eq!((moved["start_time"].as_str(), moved["end_time"].as_str()), (Some("13:00"), Some("13:30")));
    assert!(c.book(&c.patient, &c.doctor, None, &day, "11:00", "11:30").is_ok());
    assert!(c.audit_has("appointment.move"));

    // Stale versions are conflicts, not silent overwrites.
    assert_eq!(
        c.t.call(
            m::APPOINTMENTS_UPDATE,
            json!({"id": a["id"], "version": a["version"], "doctor_id": c.doctor, "date": day, "start_time": "14:00", "end_time": "14:30"}),
            Some(&c.owner),
        ),
        Err(ErrorCode::Conflict)
    );
}

impl Clinic4 {
    fn audit_has(&self, action: &str) -> bool {
        self.t.audit_actions(&self.owner).iter().any(|a| a == action)
    }
}

#[test]
fn appointment_status_workflow_queue_and_walk_ins() {
    let c = clinic4();
    let day = plus_days(&c.today, 5);
    let future = c.book(&c.patient, &c.doctor, None, &day, "09:00", "09:30").unwrap();
    // Only today's patients can arrive.
    let (_, field, rule) = {
        let r = rule_of(
            &c.t,
            m::APPOINTMENTS_SET_STATUS,
            json!({"id": future["id"], "version": future["version"], "status": "checked_in"}),
            &c.owner,
        );
        (r.0, r.1, r.2)
    };
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("status"), Some("not_today")));

    let a = c.book(&c.patient, &c.doctor, None, &c.today, "09:00", "09:30").unwrap();
    let b = c.book(&c.patient2, &c.doctor, None, &c.today, "09:30", "10:00").unwrap();
    // No jumping ahead.
    assert_eq!(c.status(&a, "in_treatment"), Err(ErrorCode::Validation));
    assert_eq!(c.status(&a, "completed"), Err(ErrorCode::Validation));
    assert_eq!(c.status(&a, "rescheduled"), Err(ErrorCode::Validation));

    let a = c.status(&a, "confirmed").unwrap();
    // Second patient arrives first: queue numbers follow arrival, not appointment time.
    let b = c.status(&b, "checked_in").unwrap();
    let a = c.status(&a, "checked_in").unwrap();
    assert_eq!((b["queue_number"].as_i64(), a["queue_number"].as_i64()), (Some(1), Some(2)));
    assert!(a["checked_in_at"].is_string());

    // A walk-in joins the queue already checked in, after the others.
    let w = c.t.ok(m::APPOINTMENTS_WALK_IN, json!({"patient_id": c.t.ok(m::PATIENTS_CREATE, json!({"full_name": "Walk In", "phone": "0700333333"}), Some(&c.owner))["id"], "doctor_id": c.doctor2, "reason": "pain"}), Some(&c.owner));
    assert_eq!(
        (w["status"].as_str(), w["is_walk_in"].as_bool(), w["queue_number"].as_i64()),
        (Some("checked_in"), Some(true), Some(3))
    );
    assert_eq!(w["date"].as_str(), Some(c.today.as_str()));
    // The same patient cannot be put in the queue twice.
    assert_eq!(
        c.t.call(
            m::APPOINTMENTS_WALK_IN,
            json!({"patient_id": w["patient_id"], "doctor_id": c.doctor}),
            Some(&c.owner)
        ),
        Err(ErrorCode::Validation)
    );

    // Treatment and completion.
    let b = c.status(&b, "in_treatment").unwrap();
    assert!(b["treatment_started_at"].is_string());
    let b = c.status(&b, "completed").unwrap();
    assert!(b["completed_at"].is_string());
    assert_eq!(c.status(&b, "cancelled"), Err(ErrorCode::Validation), "completed is final");
    // Finished visits cannot be edited either.
    assert_eq!(
        c.t.call(m::APPOINTMENTS_UPDATE, json!({"id": b["id"], "version": b["version"], "doctor_id": c.doctor, "date": c.today, "start_time": "09:30", "end_time": "10:00"}), Some(&c.owner)),
        Err(ErrorCode::Validation)
    );
    // Once the patient has arrived the time can no longer be moved, but notes can.
    let moved = c.t.call(m::APPOINTMENTS_UPDATE, json!({"id": a["id"], "version": a["version"], "doctor_id": c.doctor, "date": c.today, "start_time": "11:00", "end_time": "11:30"}), Some(&c.owner));
    assert_eq!(moved, Err(ErrorCode::Validation));
    // OF-035: a chair may have been auto-assigned when `a` was booked; the real UI always resends
    // the current chair explicitly on every edit (AppointmentDialog/CalendarPage.move), so this
    // notes-only edit does too — omitting it would mean "clear the chair", not "leave it alone".
    assert!(c.t.call(m::APPOINTMENTS_UPDATE, json!({"id": a["id"], "version": a["version"], "doctor_id": c.doctor, "chair_id": a["chair_id"], "date": c.today, "start_time": "09:00", "end_time": "09:30", "notes": "late"}), Some(&c.owner)).is_ok());

    // The day's queue, as the "today" page reads it.
    let today = c.t.ok(
        m::APPOINTMENTS_LIST,
        json!({"date_from": c.today, "date_to": c.today, "statuses": ["checked_in", "in_treatment"]}),
        Some(&c.owner),
    );
    assert_eq!(today.as_array().unwrap().len(), 2, "the waiting patient and the walk-in");
    // Month counts exclude cancelled visits and tell how many are still open.
    let counts =
        c.t.ok(m::APPOINTMENTS_COUNTS, json!({"date_from": c.today, "date_to": day}), Some(&c.owner));
    assert_eq!(counts[0]["date"], json!(c.today));
    assert_eq!((counts[0]["total"].as_i64(), counts[0]["open"].as_i64()), (Some(3), Some(2)));

    // Undo a mistaken "arrived" click.
    let a = c.get(&a["id"]);
    let a = c.status(&a, "confirmed").unwrap();
    assert!(a["queue_number"].is_null());

    // Status permissions: reception arrives and cancels, but only clinical staff run treatments.
    c.t.ok(m::USERS_CREATE, json!({"username": "recep1", "display_name": "r", "password": "recep-pass-12", "role": "receptionist"}), Some(&c.owner));
    c.t.ok(
        m::USERS_CREATE,
        json!({"username": "assist1", "display_name": "a", "password": "assist-pass-1", "role": "assistant"}),
        Some(&c.owner),
    );
    let recep = c.t.login("recep1", "recep-pass-12");
    let assist = c.t.login("assist1", "assist-pass-1");
    let a = c.status(&a, "checked_in").unwrap();
    let set = |token: &str, status: &str| {
        c.t.call(
            m::APPOINTMENTS_SET_STATUS,
            json!({"id": a["id"], "version": c.get(&a["id"])["version"], "status": status}),
            Some(token),
        )
    };
    assert_eq!(set(&recep, "in_treatment"), Err(ErrorCode::Forbidden));
    assert!(set(&assist, "in_treatment").is_ok());
    assert_eq!(c.t.call(m::APPOINTMENTS_CREATE, json!({"patient_id": c.patient, "doctor_id": c.doctor, "date": day, "start_time": "15:00", "end_time": "15:30"}), Some(&assist)), Err(ErrorCode::Forbidden));
    assert!(c.audit_has("appointment.walk_in") && c.audit_has("appointment.completed"));
}

#[test]
fn rescheduling_keeps_a_linked_history() {
    let c = clinic4();
    let day = plus_days(&c.today, 2);
    let old = c.book(&c.patient, &c.doctor, None, &day, "09:00", "09:30").unwrap();
    // The old slot is free for the new booking: moving 30 minutes later overlaps the old one.
    let new = c.t.ok(
        m::APPOINTMENTS_RESCHEDULE,
        json!({"id": old["id"], "version": old["version"], "doctor_id": c.doctor, "date": day, "start_time": "09:15", "end_time": "09:45"}),
        Some(&c.owner),
    );
    assert_eq!((new["status"].as_str(), new["reason"].as_str()), (Some("scheduled"), Some("check")));
    assert_eq!(new["rescheduled_from_id"], old["id"]);
    let old_now = c.get(&old["id"]);
    assert_eq!(old_now["status"], "rescheduled");
    assert_eq!(old_now["rescheduled_to_id"], new["id"]);
    // The old slot no longer blocks anybody.
    assert!(c.book(&c.patient2, &c.doctor, None, &day, "09:00", "09:15").is_ok());
    // A rescheduled record is final.
    assert_eq!(c.status(&old_now, "cancelled"), Err(ErrorCode::Validation));
    assert_eq!(
        c.t.call(m::APPOINTMENTS_RESCHEDULE, json!({"id": old["id"], "version": old_now["version"], "doctor_id": c.doctor, "date": day, "start_time": "14:00", "end_time": "14:30"}), Some(&c.owner)),
        Err(ErrorCode::Validation)
    );
    // The patient's history lists both, newest first.
    let history = c.t.ok(m::APPOINTMENTS_LIST, json!({"patient_id": c.patient}), Some(&c.owner));
    assert_eq!(history.as_array().unwrap().len(), 2);
}

#[test]
fn follow_ups_recalls_and_the_call_list() {
    let c = clinic4();
    let day = plus_days(&c.today, 1);
    let list = |statuses: Value| c.t.ok(m::RECALLS_LIST, json!({"statuses": statuses}), Some(&c.owner));

    // A recurring recall (scaling every 6 months) on the call list.
    let r = c.t.ok(m::RECALLS_CREATE, json!({"patient_id": c.patient, "kind": "cleaning", "due_date": "2026-11-01", "repeat_months": 6, "note": "scaling"}), Some(&c.owner));
    assert_eq!(r["status"], "pending");
    let (_, field, rule) = rule_of(
        &c.t,
        m::RECALLS_CREATE,
        json!({"patient_id": c.patient, "kind": "cleaning", "due_date": "2026-11-01", "repeat_months": 99}),
        &c.owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("repeat_months"), Some("repeat_months_range")));
    assert_eq!(list(json!([]))["total"], 1);

    // Phoning the patient records the call and keeps them on the list.
    let r = c.t.ok(
        m::RECALLS_SET_STATUS,
        json!({"id": r["id"], "version": r["version"], "status": "contacted", "note": "will call back"}),
        Some(&c.owner),
    );
    assert_eq!(
        (r["status"].as_str(), r["contact_note"].as_str()),
        (Some("contacted"), Some("will call back"))
    );
    assert!(r["last_contacted_at"].is_string());
    assert_eq!(list(json!([]))["total"], 1);
    // `booked` and `done` are not manual moves.
    assert_eq!(
        c.t.call(
            m::RECALLS_SET_STATUS,
            json!({"id": r["id"], "version": r["version"], "status": "done"}),
            Some(&c.owner)
        ),
        Err(ErrorCode::Validation)
    );

    // Booking from the call list takes the patient off it.
    let appt = c.t.ok(
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient, "doctor_id": c.doctor, "date": c.today, "start_time": "09:00", "end_time": "09:30", "recall_id": r["id"]}),
        Some(&c.owner),
    );
    assert_eq!(list(json!([]))["total"], 0);
    assert_eq!(list(json!(["booked"]))["items"][0]["appointment_id"], appt["id"]);
    // Someone else's recall cannot be used.
    let other = c.t.ok(
        m::RECALLS_CREATE,
        json!({"patient_id": c.patient2, "kind": "checkup", "due_date": day}),
        Some(&c.owner),
    );
    let (_, field, _) = rule_of(
        &c.t,
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient, "doctor_id": c.doctor, "date": day, "start_time": "09:00", "end_time": "09:30", "recall_id": other["id"]}),
        &c.owner,
    );
    assert_eq!(field.as_deref(), Some("recall_id"));

    // Cancelling the visit puts the patient back on the list.
    let cancelled = c.status(&appt, "cancelled").unwrap();
    assert_eq!(cancelled["status"], "cancelled");
    let back = list(json!(["pending"]));
    assert_eq!(
        back["items"].as_array().unwrap().iter().filter(|x| x["patient_id"] == json!(c.patient)).count(),
        1
    );

    // Complete the cycle: book, treat, complete → done, and the recurring recall schedules the next.
    let back_recall = back["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["patient_id"] == json!(c.patient))
        .unwrap()
        .clone();
    let appt = c.t.ok(
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient, "doctor_id": c.doctor, "date": c.today, "start_time": "10:00", "end_time": "10:30", "recall_id": back_recall["id"]}),
        Some(&c.owner),
    );
    let appt = c.status(&appt, "checked_in").unwrap();
    let appt = c.status(&appt, "in_treatment").unwrap();
    c.t.ok(
        m::APPOINTMENTS_SET_STATUS,
        json!({"id": appt["id"], "version": appt["version"], "status": "completed",
               "follow_up": {"due_date": plus_days(&c.today, 14), "kind": "follow_up", "note": "check stitches"}}),
        Some(&c.owner),
    );
    assert_eq!(list(json!(["done"]))["total"], 1);
    let open = list(json!([]));
    assert_eq!(
        open["total"], 3,
        "the other patient's checkup, the doctor's follow-up and the next 6-month scaling"
    );
    let kinds: Vec<&str> =
        open["items"].as_array().unwrap().iter().map(|x| x["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"follow_up") && kinds.contains(&"cleaning") && kinds.contains(&"checkup"));
    let next = open["items"].as_array().unwrap().iter().find(|x| x["kind"] == "cleaning").unwrap();
    assert_eq!(next["repeat_months"], 6, "the recurrence continues");
    assert!(next["due_date"].as_str().unwrap() > c.today.as_str());

    // A no-show creates one call-list entry, not one per missed visit.
    let ns1 = c.book(&c.patient2, &c.doctor, None, &c.today, "12:00", "12:30").unwrap();
    c.status(&ns1, "no_show").unwrap();
    let ns2 = c.book(&c.patient2, &c.doctor, None, &c.today, "13:00", "13:30").unwrap();
    c.status(&ns2, "no_show").unwrap();
    let no_shows = c.t.ok(m::RECALLS_LIST, json!({"patient_id": c.patient2}), Some(&c.owner));
    assert_eq!(no_shows["items"].as_array().unwrap().iter().filter(|x| x["kind"] == "no_show").count(), 1);

    // The "due within a fortnight" view.
    let soon = c.t.ok(m::RECALLS_LIST, json!({"due_until": plus_days(&c.today, 15)}), Some(&c.owner));
    assert!(soon["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x["due_date"].as_str().unwrap() <= plus_days(&c.today, 15).as_str()));
    assert!(c.audit_has("recall.booked") && c.audit_has("recall.done") && c.audit_has("recall.reopened"));
}

#[test]
fn patients_with_open_appointments_are_protected_and_merges_move_them() {
    let c = clinic4();
    let day = plus_days(&c.today, 4);
    let a = c.book(&c.patient, &c.doctor, None, &day, "09:00", "09:30").unwrap();
    c.t.ok(
        m::RECALLS_CREATE,
        json!({"patient_id": c.patient, "kind": "checkup", "due_date": day}),
        Some(&c.owner),
    );
    let patient = c.t.ok(m::PATIENTS_GET, json!({"patient_id": c.patient}), Some(&c.owner));
    let (_, field, rule) =
        rule_of(&c.t, m::PATIENTS_DELETE, json!({"id": c.patient, "version": patient["version"]}), &c.owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("id"), Some("patient_has_open_appointments")));

    // Merging P2 into P1 moves P2's visits and recalls to P1.
    let b = c.book(&c.patient2, &c.doctor, None, &day, "10:00", "10:30").unwrap();
    let p2 = c.t.ok(m::PATIENTS_GET, json!({"patient_id": c.patient2}), Some(&c.owner));
    c.t.ok(
        m::PATIENTS_MERGE,
        json!({"keep_id": c.patient, "merge_id": c.patient2, "merge_id_version": p2["version"]}),
        Some(&c.owner),
    );
    assert_eq!(c.get(&b["id"])["patient_id"], json!(c.patient));
    assert_eq!(
        c.t.ok(m::APPOINTMENTS_LIST, json!({"patient_id": c.patient}), Some(&c.owner))
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // A merged-away record cannot receive new bookings.
    let (_, field, rule) = rule_of(
        &c.t,
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient2, "doctor_id": c.doctor, "date": day, "start_time": "11:00", "end_time": "11:30"}),
        &c.owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("patient_id"), Some("patient_merged")));

    // After cancelling, the patient can be removed, and nobody keeps phoning a deleted patient.
    c.status(&a, "cancelled").unwrap();
    let b = c.get(&b["id"]);
    c.status(&b, "cancelled").unwrap();
    let patient = c.t.ok(m::PATIENTS_GET, json!({"patient_id": c.patient}), Some(&c.owner));
    c.t.ok(m::PATIENTS_DELETE, json!({"id": c.patient, "version": patient["version"]}), Some(&c.owner));
    let open = c.t.ok(m::RECALLS_LIST, json!({"patient_id": c.patient}), Some(&c.owner));
    assert_eq!(open["total"], 0);
}

#[test]
fn custom_roles_permissions_and_account_administration() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let perms = t.ok(m::PERMISSIONS_LIST, json!({}), Some(&owner));
    assert!(perms
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["code"] == "doctors.manage" && p["assignable"] == true));
    assert!(perms
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["code"] == "backup.restore" && p["assignable"] == false));

    // A "front desk" role: books appointments, sees patients — nothing financial.
    let role = t.ok(m::ROLES_CREATE, json!({"label": "پذیرش شبانه", "permissions": ["patients.view", "appointments.view", "appointments.edit"]}), Some(&owner));
    let code = s_of(&role, "code");
    assert_eq!(
        (role["is_system"].as_bool(), role["label"].as_str(), role["user_count"].as_i64()),
        (Some(false), Some("پذیرش شبانه"), Some(0))
    );
    let (_, field, rule) = rule_of(
        &t,
        m::ROLES_CREATE,
        json!({"label": "پذیرش شبانه", "permissions": ["patients.view"]}),
        &owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("label"), Some("role_label_taken")));
    let (_, field, rule) =
        rule_of(&t, m::ROLES_CREATE, json!({"label": "x", "permissions": ["backup.restore"]}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("permissions"), Some("permission_not_allowed")));
    let (_, _, rule) =
        rule_of(&t, m::ROLES_CREATE, json!({"label": "y", "permissions": ["nope.nope"]}), &owner);
    assert_eq!(rule.as_deref(), Some("permission_unknown"));
    let (_, _, rule) = rule_of(&t, m::ROLES_CREATE, json!({"label": "z", "permissions": []}), &owner);
    assert_eq!(rule.as_deref(), Some("permissions_empty"));

    // Assign it; the user has exactly those permissions.
    let user = t.ok(
        m::USERS_CREATE,
        json!({"username": "night1", "display_name": "n", "password": "night-pass-12", "role": code}),
        Some(&owner),
    );
    let token = t.login("night1", "night-pass-12");
    assert!(t.call(m::PATIENTS_LIST, json!({}), Some(&token)).is_ok());
    assert_eq!(
        t.call(m::PATIENTS_CREATE, json!({"full_name": "x"}), Some(&token)),
        Err(ErrorCode::Forbidden)
    );
    assert!(t.call(m::SETTINGS_GET, json!({}), Some(&token)).is_ok(), "reading settings is open");
    assert_eq!(t.call(m::USERS_LIST, json!({}), Some(&token)), Err(ErrorCode::Forbidden));
    assert_eq!(
        t.ok(m::ROLES_LIST, json!({}), Some(&owner))
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["code"] == json!(code))
            .unwrap()["user_count"],
        1
    );

    // Editing the role changes what the signed-in user may do, immediately.
    let updated = t.ok(m::ROLES_UPDATE, json!({"code": code, "version": role["version"], "label": "پذیرش شبانه", "permissions": ["patients.view", "patients.edit", "appointments.view"]}), Some(&owner));
    assert_eq!(updated["permissions"].as_array().unwrap().len(), 3);
    assert!(t.call(m::PATIENTS_CREATE, json!({"full_name": "x"}), Some(&token)).is_ok());
    assert_eq!(
        t.call(m::APPOINTMENTS_CREATE, json!({"patient_id": "x"}), Some(&token)),
        Err(ErrorCode::Forbidden)
    );
    // Stale edits conflict.
    assert_eq!(
        t.call(
            m::ROLES_UPDATE,
            json!({"code": code, "version": role["version"], "label": "q", "permissions": ["patients.view"]}),
            Some(&owner)
        ),
        Err(ErrorCode::Conflict)
    );

    // OF-028: built-in roles (not the owner) can be changed and put back to their defaults; the owner cannot.
    let reception = t
        .ok(m::ROLES_LIST, json!({}), Some(&owner))
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["code"] == json!("receptionist"))
        .unwrap()
        .clone();
    let changed = t.ok(m::ROLES_UPDATE, json!({"code": "receptionist", "version": reception["version"], "label": "", "permissions": ["patients.view"]}), Some(&owner));
    assert_eq!(changed["customized"], json!(true));
    assert_eq!(changed["permissions"], json!(["patients.view"]));
    let back =
        t.ok(m::ROLES_RESET, json!({"code": "receptionist", "version": changed["version"]}), Some(&owner));
    assert_eq!(back["customized"], json!(false));
    assert!(back["permissions"].as_array().unwrap().len() > 1, "defaults are back");
    let (_, _, rule) = rule_of(
        &t,
        m::ROLES_UPDATE,
        json!({"code": "owner", "version": 1, "label": "x", "permissions": ["patients.view"]}),
        &owner,
    );
    assert_eq!(rule.as_deref(), Some("owner_immutable"));
    let (_, _, rule) =
        rule_of(&t, m::ROLES_DELETE, json!({"code": code, "version": updated["version"]}), &owner);
    assert_eq!(rule.as_deref(), Some("role_in_use"));

    // Reset the user's password: old password and old sessions stop working; the lockout is cleared.
    for _ in 0..crate::auth::MAX_FAILED_ATTEMPTS {
        let _ = t.call(m::AUTH_LOGIN, json!({"username": "night1", "password": "wrong"}), None);
    }
    assert_eq!(
        t.call(m::AUTH_LOGIN, json!({"username": "night1", "password": "night-pass-12"}), None),
        Err(ErrorCode::AccountLocked)
    );
    let (_, field, rule) =
        rule_of(&t, m::USERS_RESET_PASSWORD, json!({"id": user["id"], "new_password": "short"}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("new_password"), Some("password_too_short")));
    t.ok(
        m::USERS_RESET_PASSWORD,
        json!({"id": user["id"], "new_password": "brand-new-pass-1"}),
        Some(&owner),
    );
    assert_eq!(t.call(m::PATIENTS_LIST, json!({}), Some(&token)), Err(ErrorCode::SessionExpired));
    assert_eq!(
        t.call(m::AUTH_LOGIN, json!({"username": "night1", "password": "night-pass-12"}), None),
        Err(ErrorCode::InvalidCredentials)
    );
    assert!(t
        .call(m::AUTH_LOGIN, json!({"username": "night1", "password": "brand-new-pass-1"}), None)
        .is_ok());
    // Resetting needs the permission.
    assert_eq!(
        t.call(
            m::USERS_RESET_PASSWORD,
            json!({"id": user["id"], "new_password": "another-pass-12"}),
            Some(&t.login("night1", "brand-new-pass-1"))
        ),
        Err(ErrorCode::Forbidden)
    );

    // Unlock without changing the password.
    for _ in 0..crate::auth::MAX_FAILED_ATTEMPTS {
        let _ = t.call(m::AUTH_LOGIN, json!({"username": "night1", "password": "wrong"}), None);
    }
    t.ok(m::USERS_UNLOCK, json!({"id": user["id"]}), Some(&owner));
    assert!(t
        .call(m::AUTH_LOGIN, json!({"username": "night1", "password": "brand-new-pass-1"}), None)
        .is_ok());

    // An administrator cannot deactivate their own account; deleting the role works once it is unused.
    let me = t
        .ok(m::USERS_LIST, json!({}), Some(&owner))
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "owner")
        .unwrap()
        .clone();
    let (_, field, rule) = rule_of(
        &t,
        m::USERS_UPDATE,
        json!({"id": me["id"], "version": me["version"], "display_name": "x", "role": "owner", "is_active": false}),
        &owner,
    );
    assert_eq!(field.as_deref(), Some("is_active"));
    assert!(rule.is_some());
    let user = t
        .ok(m::USERS_LIST, json!({}), Some(&owner))
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "night1")
        .unwrap()
        .clone();
    t.ok(m::USERS_UPDATE, json!({"id": user["id"], "version": user["version"], "display_name": "n", "role": "assistant", "is_active": true}), Some(&owner));
    let role_now = t
        .ok(m::ROLES_LIST, json!({}), Some(&owner))
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["code"] == json!(code))
        .unwrap()
        .clone();
    t.ok(m::ROLES_DELETE, json!({"code": code, "version": role_now["version"]}), Some(&owner));
    assert!(t
        .ok(m::ROLES_LIST, json!({}), Some(&owner))
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["code"] != json!(code)));
    let actions = t.audit_actions(&owner);
    for a in ["role.create", "role.update", "role.delete", "user.reset_password", "user.unlock"] {
        assert!(actions.contains(&a.to_string()), "{a} is audited");
    }
}

#[test]
fn migration_to_scheduling_keeps_existing_data() {
    // Build a database at schema v5 with a patient and an attachment, then open it with this version.
    let dir = tempfile::tempdir().unwrap();
    let config = Config::new(Environment::Test, dir.path().to_path_buf());
    let recovery = RecoveryKey::generate();
    let protector = default_protector();
    let path = config.db_path();
    {
        let (mut conn, _key) = db::create(&path, protector.as_ref(), &recovery).unwrap();
        let v5: Vec<db::Migration> = db::MIGRATIONS
            .iter()
            .filter(|m| m.version <= 5)
            .map(|m| db::Migration { version: m.version, name: m.name, sql: m.sql })
            .collect();
        db::migrate(&mut conn, &v5, &config.backup_dir()).unwrap();
        conn.execute_batch(
            "INSERT INTO db_meta(key, value) VALUES ('clinic_name', 'Old'), ('default_language', 'fa'), ('clinic_id', 'x'),
                    ('created_at', '2026-01-01T00:00:00.000Z'), ('created_with_version', '0.3.0');
             INSERT INTO patient(id, patient_number, full_name, registration_date, created_at, updated_at)
             VALUES ('p1', 'P-000001', 'Old Patient', '2026-01-01', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');",
        )
        .unwrap();
    }
    let core = Core::open(config).unwrap();
    assert!(core.is_set_up());
    let schema: i64 = core.with_db(|o| db::schema_version(&o.conn)).unwrap();
    assert_eq!(schema, db::MIGRATIONS.last().unwrap().version);
    let n: i64 =
        core.with_db(|o| Ok(o.conn.query_row("SELECT COUNT(*) FROM patient", [], |r| r.get(0))?)).unwrap();
    assert_eq!(n, 1, "existing patients survive the migration");
    let tables: i64 = core
        .with_db(|o| {
            Ok(o.conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name IN ('doctor', 'chair', 'appointment', 'recall', 'doctor_leave')",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(tables, 5);
}

#[test]
fn thumbnails_are_made_by_the_core_and_cover_older_files() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let patient = t.ok(m::PATIENTS_CREATE, json!({"full_name": "X-ray Patient"}), Some(&owner));
    let pid = s_of(&patient, "id");
    let img = image::RgbaImage::from_pixel(1600, 1200, image::Rgba([10, 120, 200, 255]));
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let up = |name: &str, bytes: &[u8]| {
        t.ok(
            m::ATTACHMENTS_UPLOAD,
            json!({"patient_id": pid, "kind": "xray", "file_name": name, "data_base64": data_encoding::BASE64.encode(bytes)}),
            Some(&owner),
        )
    };
    let x = up("panorama.png", &png);
    assert_eq!(x["has_thumbnail"], true, "made at save time");
    let pdf = up("report.pdf", b"%PDF-1.4 fake");
    assert_eq!(pdf["has_thumbnail"], false);

    let thumbs = t.ok(m::ATTACHMENTS_THUMBNAILS, json!({"patient_id": pid}), Some(&owner));
    let by = |id: &Value| thumbs.as_array().unwrap().iter().find(|x| x["id"] == *id).unwrap().clone();
    let url = by(&x["id"])["data_url"].as_str().unwrap().to_string();
    assert!(url.starts_with("data:image/jpeg;base64,"));
    assert!(by(&pdf["id"])["data_url"].is_null());
    let small =
        data_encoding::BASE64.decode(url.trim_start_matches("data:image/jpeg;base64,").as_bytes()).unwrap();
    assert!(small.len() < png.len(), "the thumbnail is far smaller than the file");
    // The list asks for the small file, the viewer for the original.
    let thumb_file = t.ok(m::ATTACHMENTS_FILE, json!({"id": x["id"], "thumbnail": true}), Some(&owner));
    assert!(thumb_file["data_url"].as_str().unwrap().starts_with("data:image/jpeg"));
    let full = t.ok(m::ATTACHMENTS_FILE, json!({"id": x["id"]}), Some(&owner));
    assert!(full["data_url"].as_str().unwrap().starts_with("data:image/png"));

    // A file saved before thumbnails existed gets one on first request.
    t.core
        .with_db(|o| {
            o.conn.execute("UPDATE patient_attachment SET thumbnail_sha256 = NULL, has_thumbnail = 0", [])?;
            Ok(())
        })
        .unwrap();
    let again = t.ok(m::ATTACHMENTS_THUMBNAILS, json!({"patient_id": pid}), Some(&owner));
    assert!(again.as_array().unwrap().iter().any(|e| e["id"] == x["id"] && e["data_url"].is_string()));
    assert_eq!(
        t.ok(m::ATTACHMENTS_LIST, json!({"patient_id": pid}), Some(&owner))
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["has_thumbnail"] == true)
            .count(),
        1
    );
}

fn xlsx_bytes(rows: &[Vec<&str>]) -> Vec<u8> {
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            // Numbers stay numbers, like phone numbers typed into Excel.
            match v.parse::<f64>() {
                Ok(n) if !v.starts_with('0') => ws.write_number(r as u32, c as u16, n).unwrap(),
                _ => ws.write_string(r as u32, c as u16, *v).unwrap(),
            };
        }
    }
    wb.save_to_buffer().unwrap()
}

#[test]
fn excel_and_csv_import_with_column_mapping() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let b64 = |b: &[u8]| data_encoding::BASE64.encode(b);

    // An Excel file with Dari headers in a different order than our template, and an extra column.
    let book = xlsx_bytes(&[
        vec!["شماره", "نام و تخلص", "نام پدر", "جنسیت", "ولایت", "تاریخ تولد", "یادداشت", "ستون نامعلوم"],
        vec!["700123456", "احمد ظاهر", "کریم", "مرد", "کابل", "1370/05/10", "اولین بار", "؟"],
        vec!["0799000111", "زرغونه", "", "زن", "هرات", "1990-01-01", "", ""],
        vec!["12", "شماره بد", "", "", "", "", "", ""],
        vec!["", "", "", "", "", "", "", ""],
        vec!["0788000222", "جنسیت ناشناخته", "", "؟؟", "", "", "", ""],
    ]);
    let info = t.ok(
        m::PATIENTS_IMPORT_INSPECT,
        json!({"file_base64": b64(&book), "file_name": "bimaran.xlsx"}),
        Some(&owner),
    );
    assert_eq!(info["file_kind"], "xlsx");
    assert_eq!(info["headers"][1], "نام و تخلص");
    assert_eq!(info["total_rows"], 4, "blank rows are not counted");
    assert_eq!(info["sample_rows"].as_array().unwrap().len(), 4);
    let guessed: Vec<(u64, String)> = info["suggested"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["column"].as_u64().unwrap(), m["field"].as_str().unwrap().to_string()))
        .collect();
    for want in [
        (0, "phone"),
        (1, "full_name"),
        (2, "father_name"),
        (3, "gender"),
        (4, "province"),
        (5, "date_of_birth"),
        (6, "notes"),
    ] {
        assert!(guessed.contains(&(want.0, want.1.to_string())), "{want:?} in {guessed:?}");
    }
    assert_eq!(guessed.len(), 7, "the unknown column stays unmapped");

    // Preview with the automatic guess: nothing is written, every problem names its row and field.
    let preview = t.ok(
        m::PATIENTS_IMPORT,
        json!({"file_base64": b64(&book), "file_name": "bimaran.xlsx", "commit": false}),
        Some(&owner),
    );
    assert_eq!(
        (preview["total"].as_i64(), preview["skipped"].as_i64(), preview["imported"].as_i64()),
        (Some(4), Some(2), Some(0))
    );
    let errs = preview["errors"].as_array().unwrap();
    assert!(errs
        .iter()
        .any(|e| e["row_number"] == 4 && e["field"] == "phone" && e["rule"] == "phone_format"));
    assert!(errs.iter().any(|e| e["row_number"] == 6 && e["field"] == "gender"));
    assert_eq!(t.ok(m::PATIENTS_LIST, json!({}), Some(&owner))["total"], 0);

    // Commit imports the good rows, with gender, province, Shamsi birth date and a numeric phone.
    let done = t.ok(
        m::PATIENTS_IMPORT,
        json!({"file_base64": b64(&book), "file_name": "bimaran.xlsx", "commit": true}),
        Some(&owner),
    );
    assert_eq!((done["imported"].as_i64(), done["skipped"].as_i64()), (Some(2), Some(2)));
    let list = t.ok(m::PATIENTS_LIST, json!({"query": "احمد"}), Some(&owner));
    let p = &list["items"][0];
    assert_eq!(p["phone"], "700123456");
    assert_eq!(p["father_name"], "کریم");
    assert_eq!(p["date_of_birth"], "1991-08-01", "10 Saratan 1370 is 1 August 1991");
    assert_eq!(p["notes"], "اولین بار");
    assert_eq!(p["gender_id"], json!(crate::ids::seed_id("reference_item", "gender/male")));
    assert!(p["province_id"].is_string());

    // A CSV with a custom column order and `;` as separator, mapped by hand.
    let csv = "Tel;Patient;Years\n0700555111;Mapped Person;41\n0700555222;Too Old;300\n";
    let mapping = json!([{"column": 0, "field": "phone"}, {"column": 1, "field": "full_name"}, {"column": 2, "field": "approximate_age"}]);
    let r = t.ok(
        m::PATIENTS_IMPORT,
        json!({"file_base64": b64(csv.as_bytes()), "file_name": "x.csv", "mapping": mapping, "commit": true}),
        Some(&owner),
    );
    assert_eq!((r["imported"].as_i64(), r["skipped"].as_i64()), (Some(1), Some(1)));
    assert!(r["errors"][0]["field"] == "approximate_age");

    // A mapping with no name column, or one mapping a field twice, is refused before reading rows.
    let bad = json!([{"column": 0, "field": "phone"}]);
    let (code, field, rule) = rule_of(
        &t,
        m::PATIENTS_IMPORT,
        json!({"file_base64": b64(csv.as_bytes()), "file_name": "x.csv", "mapping": bad}),
        &owner,
    );
    assert_eq!(
        (code, field.as_deref(), rule.as_deref()),
        (ErrorCode::Validation, Some("mapping"), Some("import_no_name_column"))
    );
    let twice = json!([{"column": 0, "field": "full_name"}, {"column": 1, "field": "full_name"}]);
    let (_, _, rule) = rule_of(
        &t,
        m::PATIENTS_IMPORT,
        json!({"file_base64": b64(csv.as_bytes()), "file_name": "x.csv", "mapping": twice}),
        &owner,
    );
    assert_eq!(rule.as_deref(), Some("import_column"));

    // Old binary Excel files and garbage get a clear message, not a crash.
    let (_, _, rule) = rule_of(
        &t,
        m::PATIENTS_IMPORT_INSPECT,
        json!({"file_base64": b64(&[0xD0, 0xCF, 0x11, 0xE0, 1, 2, 3]), "file_name": "old.xls"}),
        &owner,
    );
    assert_eq!(rule.as_deref(), Some("import_file_type"));
    let (_, _, rule) = rule_of(
        &t,
        m::PATIENTS_IMPORT_INSPECT,
        json!({"file_base64": b64(b"PK\x03\x04 not a workbook"), "file_name": "bad.xlsx"}),
        &owner,
    );
    assert_eq!(rule.as_deref(), Some("import_file_read"));
}

#[test]
fn estimated_age_is_kept_as_a_birth_year_and_an_exact_date_wins() {
    // OF-019: an estimated age is stored as the estimated birth year (so it stays right
    // as years pass); an exact birth date replaces it and the age is then computed from it.
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");

    let p = t.ok(m::PATIENTS_CREATE, json!({"full_name": "Age Only", "approximate_age": 28}), Some(&owner));
    assert_eq!(p["approximate_age"], 28);
    assert!(p["date_of_birth"].is_null());

    let update = json!({
        "id": p["id"], "version": p["version"], "status": "active", "full_name": "Age Only",
        "date_of_birth": "1990-05-20", "approximate_age": null
    });
    let exact = t.ok(m::PATIENTS_UPDATE, update, Some(&owner));
    assert_eq!(exact["date_of_birth"], "1990-05-20");
    assert!(exact["approximate_age"].is_null(), "an exact date leaves no estimated age behind");

    let back = json!({
        "id": exact["id"], "version": exact["version"], "status": "active", "full_name": "Age Only",
        "date_of_birth": null, "approximate_age": 40
    });
    let again = t.ok(m::PATIENTS_UPDATE, back, Some(&owner));
    assert_eq!(again["approximate_age"], 40);
    assert!(again["date_of_birth"].is_null());
}

#[test]
fn export_writes_an_excel_file_that_reads_back_with_text_intact() {
    // OF-022 / OF-043: the export is a real .xlsx; Persian names and phone numbers
    // (with their leading zero) come back exactly, as the import reads them.
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    t.ok(m::PATIENTS_CREATE, json!({"full_name": "حسین احمدی", "phone": "0790909090"}), Some(&owner));

    let exp = t.ok(m::PATIENTS_EXPORT, json!({}), Some(&owner));
    assert_eq!(exp["rows"], 1);
    let path = exp["file_path"].as_str().unwrap();
    assert!(path.ends_with(".xlsx") && std::path::Path::new(path).exists(), "the file is written to disk");

    let b64 = |b: &[u8]| data_encoding::BASE64.encode(b);
    let bytes = std::fs::read(path).unwrap();
    let info = t.ok(
        m::PATIENTS_IMPORT_INSPECT,
        json!({"file_base64": b64(&bytes), "file_name": "export.xlsx"}),
        Some(&owner),
    );
    assert_eq!(info["file_kind"], "xlsx");
    assert_eq!(info["headers"][1], "نام و تخلص");
    assert_eq!(info["sample_rows"][0][1], "حسین احمدی");
    assert_eq!(info["sample_rows"][0][3], "0790909090");
}

#[test]
fn excel_without_a_title_row_is_read_as_data_and_can_be_overridden() {
    // OF-042: `ALI` / `790909090` is a patient, not a title row. The user can still say
    // "the first row is a title" and the rows then start one line lower.
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let b64 = |b: &[u8]| data_encoding::BASE64.encode(b);
    let book = xlsx_bytes(&[vec!["ALI", "790909090"], vec!["Karim", "790000001"]]);

    let info = t.ok(
        m::PATIENTS_IMPORT_INSPECT,
        json!({"file_base64": b64(&book), "file_name": "no-titles.xlsx"}),
        Some(&owner),
    );
    assert_eq!(info["has_header"], false);
    assert_eq!(info["headers"][0], "ستون 1");
    assert_eq!(info["sample_rows"][0][0], "ALI");
    assert_eq!(info["total_rows"], 2);

    let forced = t.ok(
        m::PATIENTS_IMPORT_INSPECT,
        json!({"file_base64": b64(&book), "file_name": "no-titles.xlsx", "has_header": true}),
        Some(&owner),
    );
    assert_eq!(forced["has_header"], true);
    assert_eq!(forced["total_rows"], 1);

    let mapping = json!([{"column": 0, "field": "full_name"}, {"column": 1, "field": "phone"}]);
    let r = t.ok(
        m::PATIENTS_IMPORT,
        json!({"file_base64": b64(&book), "file_name": "no-titles.xlsx", "mapping": mapping, "commit": true}),
        Some(&owner),
    );
    assert_eq!(r["imported"], 2);
}

#[test]
fn bookings_take_a_free_chair_and_refuse_past_times() {
    // OF-035: with no chair chosen, each booking takes a different free chair.
    // OF-036: a time that has already passed cannot be booked.
    let c = clinic4();
    let day = c.today.clone();
    let a = c.book(&c.patient, &c.doctor, None, &day, "09:00", "09:30").unwrap();
    let b = c.book(&c.patient2, &c.doctor2, None, &day, "09:00", "09:30").unwrap();
    assert_eq!(a["chair_id"], json!(c.chair));
    assert_eq!(b["chair_id"], json!(c.chair2));

    let (_, field, rule) = rule_of(
        &c.t,
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient, "doctor_id": c.doctor, "date": day, "start_time": "05:00", "end_time": "05:30"}),
        &c.owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("start_time"), Some("appointment_in_past")));
}

#[test]
fn calendar_drag_step_is_a_setting_and_moves_never_go_into_the_past() {
    // OF-044: the drag step is 5 minutes unless the clinic picks 1, 10 or 15.
    let c = clinic4();
    let mut s = c.t.ok(m::SETTINGS_GET, json!({}), Some(&c.owner));
    assert_eq!(s["calendar_snap_minutes"], 5);
    s["calendar_snap_minutes"] = json!(7);
    let (_, field, rule) = rule_of(&c.t, m::SETTINGS_UPDATE, s.clone(), &c.owner);
    assert_eq!(
        (field.as_deref(), rule.as_deref()),
        (Some("calendar_snap_minutes"), Some("calendar_snap_range"))
    );
    s["calendar_snap_minutes"] = json!(15);
    assert_eq!(c.t.ok(m::SETTINGS_UPDATE, s, Some(&c.owner))["calendar_snap_minutes"], 15);

    // A visit moved to an exact minute keeps it; moving it to a time already gone is refused.
    let a = c.book(&c.patient, &c.doctor, None, &c.today, "15:40", "16:10").unwrap();
    let moved = |date: &str, start: &str, end: &str, version: &Value| {
        json!({"id": a["id"], "version": version, "doctor_id": c.doctor, "chair_id": a["chair_id"], "date": date,
               "start_time": start, "end_time": end, "reason": "check", "notes": null})
    };
    let m1 = c.t.ok(m::APPOINTMENTS_UPDATE, moved(&c.today, "17:10", "17:40", &a["version"]), Some(&c.owner));
    assert_eq!((m1["start_time"].as_str(), m1["end_time"].as_str()), (Some("17:10"), Some("17:40")));
    let (_, field, rule) =
        rule_of(&c.t, m::APPOINTMENTS_UPDATE, moved(&c.today, "05:10", "05:40", &m1["version"]), &c.owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("start_time"), Some("appointment_in_past")));
}

#[test]
fn tooth_numbering_defaults_to_quadrant_and_keeps_the_clinics_choice() {
    // v0.6.1: a clinic that never chose sees 1–8 per quadrant (baby teeth I–V); FDI stays one choice away.
    let c = clinic4();
    let mut s = c.t.ok(m::SETTINGS_GET, json!({}), Some(&c.owner));
    assert_eq!(s["tooth_numbering"], "quadrant");
    for n in ["fdi", "palmer", "universal", "quadrant"] {
        s["tooth_numbering"] = json!(n);
        s = c.t.ok(m::SETTINGS_UPDATE, s, Some(&c.owner));
        assert_eq!(s["tooth_numbering"], n);
        assert_eq!(c.t.ok(m::SETTINGS_GET, json!({}), Some(&c.owner))["tooth_numbering"], n);
    }
}

#[test]
fn walk_in_outside_working_hours_needs_confirmation() {
    // OF-037: a walk-in outside the doctor's hours is refused until reception confirms it, and the
    // confirmation is audited. The doctor's hours are set to one minute a day, so "now" is always outside.
    let c = clinic4();
    let doc =
        c.t.ok(m::DOCTORS_LIST, json!({}), Some(&c.owner))
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == json!(c.doctor))
            .unwrap()
            .clone();
    let hours: Vec<Value> = (0..7).map(|d| json!({"day": d, "start": "00:00", "end": "00:01"})).collect();
    c.t.ok(
        m::DOCTORS_SET_SCHEDULE,
        json!({"doctor_id": c.doctor, "version": doc["version"], "hours": hours, "breaks": [], "chair_ids": []}),
        Some(&c.owner),
    );

    let walk = json!({"patient_id": c.patient2, "doctor_id": c.doctor, "reason": "pain"});
    let (_, _, rule) = rule_of(&c.t, m::APPOINTMENTS_WALK_IN, walk.clone(), &c.owner);
    assert_eq!(rule.as_deref(), Some("outside_working_hours"));

    let mut confirmed = walk;
    confirmed["override_schedule"] = json!(true);
    let w = c.t.ok(m::APPOINTMENTS_WALK_IN, confirmed, Some(&c.owner));
    assert_eq!(w["is_walk_in"], json!(true));
    assert!(c.audit_has("appointment.walk_in"));
}

#[test]
fn form_drafts_are_kept_per_form_and_must_be_objects() {
    // OF-020 (rule 14): a half-filled form is kept until it is saved or cancelled.
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");

    let none = t.ok(m::DRAFTS_GET, json!({"form_key": "patient.create"}), Some(&owner));
    assert!(none["data_json"].is_null());

    let text = "{\"full_name\":\"احمد\"}";
    t.ok(m::DRAFTS_SAVE, json!({"form_key": "patient.create", "data_json": text}), Some(&owner));
    let got = t.ok(m::DRAFTS_GET, json!({"form_key": "patient.create"}), Some(&owner));
    assert_eq!(got["data_json"], text);
    assert!(got["updated_at"].is_string());

    t.ok(m::DRAFTS_DELETE, json!({"form_key": "patient.create"}), Some(&owner));
    let gone = t.ok(m::DRAFTS_GET, json!({"form_key": "patient.create"}), Some(&owner));
    assert!(gone["data_json"].is_null());

    assert_eq!(
        t.call(m::DRAFTS_SAVE, json!({"form_key": "x", "data_json": "[1,2]"}), Some(&owner)),
        Err(ErrorCode::Validation),
        "a draft is a JSON object"
    );
}

// ───────────────────────────── Phase 5A: documents, prescriptions, catalog ─────────────────────────────

/// A checklist question's id by code.
fn qid(t: &T, token: &str, code: &str) -> Value {
    question(t, token, code)["id"].clone()
}

fn drug_line(t: &T, token: &str, code: &str) -> Value {
    let d = t
        .ok(m::DRUGS_LIST, json!({}), Some(token))
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == code)
        .unwrap_or_else(|| panic!("drug {code}"))
        .clone();
    json!({"drug_id": d["id"], "name": d["name"], "form": d["form"], "strength": d["strength"], "quantity": d["quantity"],
           "dose": d["dose"], "times_per_day": d["times_per_day"], "timing": d["timing"], "days": d["days"],
           "as_needed": d["as_needed"]})
}

#[test]
fn prescriptions_are_numbered_warn_about_allergies_and_reprint_without_a_new_number() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    // Penicillin allergy and a pregnancy in the checklist.
    t.ok(
        m::MEDICAL_HISTORY_UPDATE,
        json!({"patient_id": c.patient, "version": 0, "answers": [
            {"question_id": qid(t, owner, "allergy_penicillin"), "answer": "yes", "detail_text": "Amoxicillin"},
            {"question_id": qid(t, owner, "pregnant"), "answer": "yes", "detail_text": "4"}
        ]}),
        Some(owner),
    );
    let items = json!([drug_line(t, owner, "amoxicillin_500"), drug_line(t, owner, "paracetamol_500")]);
    let warnings =
        t.ok(m::PRESCRIPTIONS_CHECK, json!({"patient_id": c.patient, "items": items}), Some(owner));
    let warnings = warnings.as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0]["key"], "penicillin_allergy:0");
    assert_eq!(warnings[0]["severity"], "danger");
    assert_eq!(warnings[0]["detail"], "Amoxicillin");

    let issue = |acknowledged: Value, doctor: Value| {
        json!({"patient_id": c.patient, "doctor_id": doctor, "language": "fa", "paper": "a5",
               "content": {"kind": "prescription", "items": items, "notes": "  ", "acknowledged": acknowledged}})
    };
    // A prescription needs its doctor, and every warning must be accepted.
    let (_, field, rule) = rule_of(t, m::DOCUMENTS_ISSUE, issue(json!([]), json!(null)), owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("doctor_id"), Some("doctor_required")));
    let (_, field, rule) = rule_of(t, m::DOCUMENTS_ISSUE, issue(json!([]), json!(c.doctor)), owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("items.0"), Some("rx_warning_not_acknowledged")));

    let rx = t.ok(
        m::DOCUMENTS_ISSUE,
        issue(json!(["penicillin_allergy:0", "stale:9"]), json!(c.doctor)),
        Some(owner),
    );
    let year = {
        let d = crate::clock::local_now().date();
        crate::calendar::GregorianDate { year: d.year(), month: d.month() as u8, day: d.day() }
            .to_shamsi()
            .unwrap()
            .year
    };
    assert_eq!(rx["number"], format!("RX-{year}-000001"));
    assert_eq!(rx["kind"], "prescription");
    assert_eq!(rx["patient"]["name"], "بیمار یکم");
    assert_eq!(rx["doctor"]["name"], "دکتر احمد");
    assert_eq!(rx["content"]["notes"], json!(null));
    assert_eq!(
        rx["content"]["acknowledged"],
        json!(["penicillin_allergy:0"]),
        "only warnings that applied are kept"
    );
    assert_eq!(rx["issued_by_name"], "مالک");

    // Reprints count; they never make a new number.
    let printed = t.ok(m::DOCUMENTS_MARK_PRINTED, json!({"id": rx["id"]}), Some(owner));
    let printed = t.ok(m::DOCUMENTS_MARK_PRINTED, json!({"id": printed["id"], "pdf": true}), Some(owner));
    assert_eq!(printed["print_count"], 2);
    assert_eq!(printed["number"], rx["number"]);
    let second =
        t.ok(m::DOCUMENTS_ISSUE, issue(json!(["penicillin_allergy:0"]), json!(c.doctor)), Some(owner));
    assert_eq!(second["number"], format!("RX-{year}-000002"));

    // Voiding needs a reason; a void document cannot be printed again.
    let (_, field, _) =
        rule_of(t, m::DOCUMENTS_VOID, json!({"id": second["id"], "version": 1, "reason": " "}), owner);
    assert_eq!(field.as_deref(), Some("reason"));
    let void = t.ok(
        m::DOCUMENTS_VOID,
        json!({"id": second["id"], "version": 1, "reason": "دوز اشتباه"}),
        Some(owner),
    );
    assert_eq!((void["status"].as_str(), void["void_reason"].as_str()), (Some("void"), Some("دوز اشتباه")));
    let (_, _, rule) = rule_of(t, m::DOCUMENTS_MARK_PRINTED, json!({"id": second["id"]}), owner);
    assert_eq!(rule.as_deref(), Some("document_void"));

    let list = t.ok(m::DOCUMENTS_LIST, json!({"patient_id": c.patient}), Some(owner));
    assert_eq!(list.as_array().unwrap().len(), 2);
    let actions = t.audit_actions(owner);
    for a in ["document.issue", "document.print", "document.pdf", "document.void"] {
        assert!(actions.contains(&a.to_string()), "{a}");
    }
}

#[test]
fn every_document_kind_is_checked_numbered_and_frozen_as_issued() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    let doc = |content: Value| {
        t.call(
            m::DOCUMENTS_ISSUE,
            json!({"patient_id": c.patient, "language": "ps", "paper": "a5", "content": content}),
            Some(owner),
        )
    };
    let cert = doc(json!({"kind": "certificate", "visit_date": c.today, "rest_days": 3})).unwrap();
    assert!(cert["number"].as_str().unwrap().starts_with("MC-"));
    assert_eq!(cert["content"]["rest_from"], json!(c.today), "rest starts on the visit day unless said");
    assert_eq!(cert["language"], "ps");
    let (_, field, rule) = rule_of(
        t,
        m::DOCUMENTS_ISSUE,
        json!({"patient_id": c.patient, "language": "fa", "paper": "a5",
               "content": {"kind": "certificate", "visit_date": c.today, "rest_days": 90}}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("rest_days"), Some("rest_days_range")));

    let ir = doc(json!({"kind": "imaging_request", "tests": ["opg", "cbct", "opg"], "teeth": "36"})).unwrap();
    assert_eq!(ir["content"]["tests"], json!(["opg", "cbct"]));
    let (_, field, _) = rule_of(
        t,
        m::DOCUMENTS_ISSUE,
        json!({"patient_id": c.patient, "language": "fa", "paper": "a5", "content": {"kind": "imaging_request", "tests": ["other"]}}),
        owner,
    );
    assert_eq!(field.as_deref(), Some("notes"), "an 'other' test must be named");

    let rf = doc(
        json!({"kind": "referral", "to": "Dr Karimi, oral surgeon", "reason": "Impacted 38", "urgent": true}),
    )
    .unwrap();
    assert!(rf["number"].as_str().unwrap().starts_with("RF-"));
    let lo = doc(json!({"kind": "lab_order", "teeth": "11 21", "work": "Zirconia crowns", "shade": "A2"}))
        .unwrap();
    assert!(lo["number"].as_str().unwrap().starts_with("LO-"));

    // A consent form written from a built-in template keeps its words even if the template changes later.
    let templates = t.ok(m::DOCUMENT_TEMPLATES_LIST, json!({"kind": "consent"}), Some(owner));
    let extraction =
        templates.as_array().unwrap().iter().find(|x| x["code"] == "consent_extraction").unwrap().clone();
    assert_eq!(extraction["paper"], "a4");
    let cf =
        doc(json!({"kind": "consent", "template_id": extraction["id"], "title": extraction["title"]["ps"],
                        "body": "متن", "teeth": "38"}))
        .unwrap();
    assert!(cf["number"].as_str().unwrap().starts_with("CF-"));

    // The record summary is frozen with what the record said that day.
    t.ok(
        m::MEDICAL_HISTORY_UPDATE,
        json!({"patient_id": c.patient, "version": 0, "answers": [
            {"question_id": qid(t, owner, "diabetes"), "answer": "yes", "detail_choice": "controlled"}]}),
        Some(owner),
    );
    c.book(&c.patient, &c.doctor, None, &c.today, "09:00", "09:30").unwrap();
    let mr = doc(json!({"kind": "record_summary", "purpose": "For Dr Karimi"})).unwrap();
    let snap = &mr["content"]["snapshot"];
    assert_eq!(snap["medical"], json!(["د شکرې ناروغي (کنټرول شوې)"]), "labels in the document's language");
    assert_eq!(snap["visits"].as_array().unwrap().len(), 1);
    assert_eq!(snap["visits"][0]["doctor"], "دکتر احمد");

    // Reception without clinical.edit cannot issue documents.
    t.ok(
        m::USERS_CREATE,
        json!({"username": "rec", "display_name": "Reception", "password": "reception-1", "role": "receptionist"}),
        Some(owner),
    );
    let rec = t.login("rec", "reception-1");
    assert_eq!(
        t.call(
            m::DOCUMENTS_ISSUE,
            json!({"patient_id": c.patient, "language": "fa", "paper": "a5", "content": {"kind": "certificate", "visit_date": c.today}}),
            Some(&rec)
        ),
        Err(ErrorCode::Forbidden)
    );
}

#[test]
fn a_signed_consent_is_scanned_back_onto_its_document() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    let cf = t.ok(
        m::DOCUMENTS_ISSUE,
        json!({"patient_id": c.patient, "language": "fa", "paper": "a4",
               "content": {"kind": "consent", "title": "رضایت‌نامه", "body": "متن"}}),
        Some(owner),
    );
    let png = data_encoding::BASE64.encode(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let mine = t.ok(
        m::ATTACHMENTS_UPLOAD,
        json!({"patient_id": c.patient, "kind": "consent_form", "file_name": "signed.png", "data_base64": png}),
        Some(owner),
    );
    let other = t.ok(
        m::ATTACHMENTS_UPLOAD,
        json!({"patient_id": c.patient2, "kind": "consent_form", "file_name": "signed.png", "data_base64": png}),
        Some(owner),
    );
    let (_, field, _) = rule_of(
        t,
        m::DOCUMENTS_ATTACH_SCAN,
        json!({"id": cf["id"], "version": 1, "attachment_id": other["id"]}),
        owner,
    );
    assert_eq!(field.as_deref(), Some("attachment_id"));
    let linked = t.ok(
        m::DOCUMENTS_ATTACH_SCAN,
        json!({"id": cf["id"], "version": 1, "attachment_id": mine["id"]}),
        Some(owner),
    );
    assert_eq!(linked["attachment_id"], mine["id"]);
}

#[test]
fn formulary_and_templates_are_the_clinics_to_edit() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let drugs = t.ok(m::DRUGS_LIST, json!({}), Some(&owner));
    assert!(drugs.as_array().unwrap().len() >= 20);
    let amox = drugs.as_array().unwrap().iter().find(|d| d["code"] == "amoxicillin_500").unwrap().clone();
    assert_eq!(amox["classes"], json!(["penicillin"]));
    assert_eq!((amox["times_per_day"].as_i64(), amox["timing"].as_str()), (Some(3), Some("after_food")));

    let made = t.ok(
        m::DRUGS_SAVE,
        json!({"name": "Benzydamine mouthwash", "form": "mouthwash", "strength": "0.15%", "classes": ["antiseptic"], "days": 5}),
        Some(&owner),
    );
    assert_eq!(made["is_system"], false);
    let (_, field, rule) =
        rule_of(&t, m::DRUGS_SAVE, json!({"name": "X", "form": "pill", "classes": []}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("form"), Some("rx_form")));
    let (_, field, rule) =
        rule_of(&t, m::DRUGS_SAVE, json!({"name": "X", "form": "tablet", "classes": ["magic"]}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("classes"), Some("drug_class_unknown")));
    let (_, field, rule) =
        rule_of(&t, m::DRUGS_SAVE, json!({"name": "X", "form": "tablet", "times_per_day": 20}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("times_per_day"), Some("rx_dose_range")));

    let rx_templates = t.ok(m::RX_TEMPLATES_LIST, json!({}), Some(&owner));
    let after_ext =
        rx_templates.as_array().unwrap().iter().find(|x| x["code"] == "after_extraction").unwrap().clone();
    assert_eq!(after_ext["name"]["fa"], "بعد از کشیدن دندان");
    assert_eq!(after_ext["items"].as_array().unwrap().len(), 3);
    assert_eq!(after_ext["items"][0]["days"], 3, "a template can shorten a medicine's default course");

    // Rewriting a built-in template marks it as the clinic's; a start-up keeps it; "reset" brings the default back.
    let list = t.ok(m::DOCUMENT_TEMPLATES_LIST, json!({"kind": "post_op"}), Some(&owner));
    let mut tpl =
        list.as_array().unwrap().iter().find(|x| x["code"] == "post_op_extraction").unwrap().clone();
    let original = tpl["body"]["fa"].clone();
    tpl["body"]["fa"] = json!("متن کلینیک");
    let saved = t.ok(m::DOCUMENT_TEMPLATES_SAVE, tpl.clone(), Some(&owner));
    assert_eq!(
        (saved["customized"].as_bool(), saved["body"]["fa"].as_str()),
        (Some(true), Some("متن کلینیک"))
    );
    let t = t.reopen();
    let owner = t.login("owner", "owner-pass-1");
    let again = t.ok(m::DOCUMENT_TEMPLATES_LIST, json!({"kind": "post_op"}), Some(&owner));
    let again = again.as_array().unwrap().iter().find(|x| x["code"] == "post_op_extraction").unwrap().clone();
    assert_eq!(again["body"]["fa"], "متن کلینیک");
    let reset = t.ok(
        m::DOCUMENT_TEMPLATES_RESET,
        json!({"id": again["id"], "version": again["version"]}),
        Some(&owner),
    );
    assert_eq!((reset["customized"].as_bool(), &reset["body"]["fa"]), (Some(false), &original));
    let (_, field, rule) = rule_of(
        &t,
        m::DOCUMENT_TEMPLATES_SAVE,
        json!({"kind": "referral", "title": {"fa": "x", "ps": "", "en": ""}, "body": {"fa": "x", "ps": "", "en": ""}, "paper": "a4"}),
        &owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("kind"), Some("template_kind")));
}

#[test]
fn doctors_have_specialties_and_a_licence_and_old_text_is_matched() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let specialties = t.ok(m::SPECIALTIES_LIST, json!({}), Some(&owner));
    let specialties = specialties.as_array().unwrap();
    assert_eq!(specialties.len(), 9);
    let ortho = specialties.iter().find(|s| s["code"] == "orthodontics").unwrap()["id"].clone();
    let surgery = specialties.iter().find(|s| s["code"] == "oral_surgery").unwrap()["id"].clone();
    let d = t.ok(
        m::DOCTORS_CREATE,
        json!({"full_name": "دکتر احمد", "color": "#0e7490", "specialty_ids": [ortho, surgery, ortho], "license_number": " MoPH-1234 "}),
        Some(&owner),
    );
    assert_eq!(d["specialty_ids"], json!([surgery, ortho]), "list order, no duplicates");
    assert_eq!(d["license_number"], "MoPH-1234");
    let (_, field, rule) = rule_of(
        &t,
        m::DOCTORS_CREATE,
        json!({"full_name": "x", "color": "#0e7490", "specialty_ids": ["nope"]}),
        &owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("specialty_ids"), Some("specialty_not_found")));

    // A doctor saved before v0.5.0 with free text "ارتودانسی" is linked on the next start.
    let old = t.ok(
        m::DOCTORS_CREATE,
        json!({"full_name": "دکتر قدیم", "color": "#f59e0b", "specialty": "ارتودانسی"}),
        Some(&owner),
    );
    assert_eq!(old["specialty_ids"], json!([]));
    let t = t.reopen();
    let owner = t.login("owner", "owner-pass-1");
    let old = t
        .ok(m::DOCTORS_LIST, json!({}), Some(&owner))
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == old["id"])
        .unwrap()
        .clone();
    assert_eq!((old["specialty_ids"].clone(), old["specialty"].clone()), (json!([ortho]), json!(null)));

    // The clinic adds its own specialty; a seeded one can be switched off but not renamed.
    let mine = t.ok(
        m::SPECIALTIES_SAVE,
        json!({"label": {"fa": "دندان‌پزشکی سالمندان", "ps": "", "en": "Geriatric"}}),
        Some(&owner),
    );
    assert_eq!(
        (mine["is_system"].as_bool(), mine["label"]["ps"].as_str()),
        (Some(false), Some("دندان‌پزشکی سالمندان"))
    );
    let cosmetic = specialties.iter().find(|s| s["code"] == "cosmetic").unwrap().clone();
    let off = t.ok(
        m::SPECIALTIES_SAVE,
        json!({"id": cosmetic["id"], "version": cosmetic["version"], "label": {"fa": "تغییر", "ps": "", "en": ""}, "is_active": false}),
        Some(&owner),
    );
    assert_eq!((off["is_active"].as_bool(), &off["label"]["fa"]), (Some(false), &cosmetic["label"]["fa"]));
}

#[test]
fn catalog_has_three_levels_validates_and_drives_booking() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    let catalog = t.ok(m::CATALOG_GET, json!({}), Some(owner));
    assert_eq!(catalog["categories"].as_array().unwrap().len(), 11);
    let services = catalog["services"].as_array().unwrap();
    let find =
        |code: &str| services.iter().find(|s| s["code"] == code).unwrap_or_else(|| panic!("{code}")).clone();
    let rct = find("END-01");
    let two = find("END-01D");
    assert_eq!(two["parent_id"], rct["id"]);
    assert_eq!(
        (two["sessions"].as_i64(), two["tooth_scope"].as_str()),
        (Some(2), Some("tooth")),
        "variants inherit"
    );
    assert_eq!(find("END-01M")["sessions"], 3);
    let endo = t
        .ok(m::SPECIALTIES_LIST, json!({}), Some(owner))
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["code"] == "endodontics")
        .unwrap()["id"]
        .clone();
    assert_eq!(two["specialty_id"], endo, "the category's specialty");
    assert!(two["consent_template_id"].is_string());
    assert_eq!(find("RST-01")["needs_surface"], true);
    assert_eq!(find("PRS-01")["lab_required"], true);
    assert_eq!(find("DX-01")["price"], 0, "prices are the clinic's to set");

    // Price it, and the codes stay unique and well-formed.
    let mut priced = two.clone();
    priced["price"] = json!(350_000); // 3 500 AFN
    let priced = t.ok(m::CATALOG_SAVE_SERVICE, priced, Some(owner));
    assert_eq!(priced["price"], 350_000);
    let mut dup = find("END-01S");
    dup["code"] = json!("end-01d");
    let (_, field, rule) = rule_of(t, m::CATALOG_SAVE_SERVICE, dup.clone(), owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("code"), Some("service_code_taken")));
    dup["code"] = json!("ریشه");
    let (_, _, rule) = rule_of(t, m::CATALOG_SAVE_SERVICE, dup.clone(), owner);
    assert_eq!(rule.as_deref(), Some("service_code_format"));
    let mut deep = find("END-01S");
    deep["parent_id"] = two["id"].clone();
    let (_, field, rule) = rule_of(t, m::CATALOG_SAVE_SERVICE, deep, owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("parent_id"), Some("service_variant_depth")));
    let mut neg = find("DX-01");
    neg["price"] = json!(-1);
    assert_eq!(rule_of(t, m::CATALOG_SAVE_SERVICE, neg, owner).2.as_deref(), Some("service_price"));

    // A new category with a new service.
    let cat = t.ok(
        m::CATALOG_SAVE_CATEGORY,
        json!({"name": {"fa": "خدمات خانگی", "ps": "", "en": "Home visits"}, "specialty_id": null}),
        Some(owner),
    );
    let svc = t.ok(
        m::CATALOG_SAVE_SERVICE,
        json!({"code": "HV-01", "category_id": cat["id"], "name": {"fa": "معاینه در خانه", "ps": "", "en": ""},
               "price": 100_000, "tooth_scope": "none"}),
        Some(owner),
    );
    assert_eq!(svc["sessions"], 1);

    // Booking for a service: free by default, only its specialty's doctors when the clinic asks for it.
    let a = t.ok(
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient, "doctor_id": c.doctor2, "date": c.today, "start_time": "09:00", "end_time": "09:30", "service_id": two["id"]}),
        Some(owner),
    );
    assert_eq!(a["service_id"], two["id"]);
    assert_eq!(a["medical_review_due"], true, "no medical history yet");
    let mut settings = t.ok(m::SETTINGS_GET, json!({}), Some(owner));
    settings["restrict_service_specialty"] = json!(true);
    t.ok(m::SETTINGS_UPDATE, settings, Some(owner));
    let mut clinic = t.ok(m::CLINIC_GET, json!({}), Some(owner));
    clinic["clinic_mode"] = json!("multi");
    t.ok(m::CLINIC_UPDATE, clinic, Some(owner));
    let (_, field, rule) = rule_of(
        t,
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient2, "doctor_id": c.doctor2, "date": c.today, "start_time": "10:00", "end_time": "10:30", "service_id": two["id"]}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("doctor_id"), Some("service_specialty")));
    let d2 = t
        .ok(m::DOCTORS_LIST, json!({}), Some(owner))
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == json!(c.doctor2))
        .unwrap()
        .clone();
    let mut d2u = d2.clone();
    d2u["specialty_ids"] = json!([endo]);
    t.ok(m::DOCTORS_UPDATE, d2u, Some(owner));
    t.ok(
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient2, "doctor_id": c.doctor2, "date": c.today, "start_time": "10:00", "end_time": "10:30", "service_id": two["id"]}),
        Some(owner),
    );
    // A move keeps the service (the calendar resends it).
    let moved = t.ok(
        m::APPOINTMENTS_UPDATE,
        json!({"id": a["id"], "version": a["version"], "doctor_id": c.doctor2, "chair_id": a["chair_id"], "date": c.today,
               "start_time": "11:00", "end_time": "11:30", "reason": null, "notes": null, "service_id": two["id"]}),
        Some(owner),
    );
    assert_eq!(moved["service_id"], two["id"]);
}

// ───────────────────────────── Phase 5B: chart, plans, sessions, notes, perio, timeline ─────────────────────────────

fn service_id(t: &T, token: &str, code: &str) -> Value {
    t.ok(m::CATALOG_GET, json!({}), Some(token))["services"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["code"] == code)
        .unwrap_or_else(|| panic!("service {code}"))["id"]
        .clone()
}

fn condition_id(t: &T, token: &str, code: &str) -> Value {
    t.ok(m::TOOTH_CONDITIONS_LIST, json!({}), Some(token))
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["code"] == code)
        .unwrap_or_else(|| panic!("condition {code}"))["id"]
        .clone()
}

fn plan_template_id(t: &T, token: &str, code: &str) -> Value {
    t.ok(m::PLAN_TEMPLATES_LIST, json!({}), Some(token))
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["code"] == code)
        .unwrap_or_else(|| panic!("plan template {code}"))["id"]
        .clone()
}

fn actions_of(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|e| e["action"].as_str().unwrap().to_string()).collect()
}

#[test]
fn dental_chart_records_findings_dentition_and_tooth_history() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    let chart = t.ok(m::CHART_GET, json!({"patient_id": c.patient}), Some(owner));
    assert_eq!(chart["dentition"], "permanent", "no age: adult teeth");
    assert_eq!(chart["dentition_chosen"], false);
    assert!(chart["findings"].as_array().unwrap().is_empty());

    // A child of 8 starts with a mixed dentition.
    let child = t.ok(m::PATIENTS_CREATE, json!({"full_name": "طفل", "approximate_age": 8}), Some(owner));
    assert_eq!(t.ok(m::CHART_GET, json!({"patient_id": child["id"]}), Some(owner))["dentition"], "mixed");

    // Caries needs its surfaces; numbers and surfaces are checked and kept in MODBL order.
    let caries = condition_id(t, owner, "caries");
    let (_, field, rule) = rule_of(
        t,
        m::CHART_ADD_FINDINGS,
        json!({"patient_id": c.patient, "condition_id": caries, "teeth": [36]}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("surfaces"), Some("tooth_surface")));
    let (_, _, rule) = rule_of(
        t,
        m::CHART_ADD_FINDINGS,
        json!({"patient_id": c.patient, "condition_id": caries, "teeth": [19], "surfaces": "O"}),
        owner,
    );
    assert_eq!(rule.as_deref(), Some("tooth_number"));
    let added = t.ok(
        m::CHART_ADD_FINDINGS,
        json!({"patient_id": c.patient, "condition_id": caries, "teeth": [37, 36], "surfaces": "do"}),
        Some(owner),
    );
    let added = added.as_array().unwrap();
    assert_eq!(added.len(), 2, "one finding per tooth");
    assert_eq!((added[0]["tooth"].as_i64(), added[0]["surfaces"].as_str()), (Some(36), Some("OD")));
    let missing = condition_id(t, owner, "missing");
    t.ok(
        m::CHART_ADD_FINDINGS,
        json!({"patient_id": c.patient, "condition_id": missing, "teeth": [18]}),
        Some(owner),
    );
    let chart = t.ok(m::CHART_GET, json!({"patient_id": c.patient}), Some(owner));
    assert_eq!(chart["findings"].as_array().unwrap().len(), 3);

    // A finding entered by mistake leaves the chart but stays in the tooth's history.
    t.ok(m::CHART_REMOVE_FINDING, json!({"id": added[1]["id"], "version": added[1]["version"]}), Some(owner));
    let chart = t.ok(m::CHART_GET, json!({"patient_id": c.patient}), Some(owner));
    assert_eq!(chart["findings"].as_array().unwrap().len(), 2);
    let history = t.ok(m::CHART_TOOTH_HISTORY, json!({"patient_id": c.patient, "tooth": 37}), Some(owner));
    assert_eq!(actions_of(&history), vec!["finding.removed", "finding.added"]);
    let other = t.ok(m::CHART_TOOTH_HISTORY, json!({"patient_id": c.patient, "tooth": 11}), Some(owner));
    assert!(other.as_array().unwrap().is_empty());

    // Dentition: a baby tooth only takes the place of an adult tooth 1 to 5.
    let (_, field, rule) = rule_of(
        t,
        m::CHART_SET_DENTITION,
        json!({"patient_id": c.patient, "dentition": "mixed", "primary_slots": [16], "version": 0}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("primary_slots"), Some("dentition_slots")));
    let chart = t.ok(
        m::CHART_SET_DENTITION,
        json!({"patient_id": c.patient, "dentition": "mixed", "primary_slots": [14, 13], "version": 0}),
        Some(owner),
    );
    assert_eq!(
        (chart["dentition"].as_str(), chart["dentition_chosen"].as_bool()),
        (Some("mixed"), Some(true))
    );
    assert_eq!(chart["primary_slots"], json!([13, 14]));
    assert_eq!(
        t.call(
            m::CHART_SET_DENTITION,
            json!({"patient_id": c.patient, "dentition": "primary", "version": 0}),
            Some(owner)
        ),
        Err(ErrorCode::Conflict),
        "stale version"
    );

    // The clinic's own condition, drawn as a badge.
    let (_, field, rule) = rule_of(
        t,
        m::TOOTH_CONDITIONS_SAVE,
        json!({"name": {"fa": "تغییر رنگ", "ps": "", "en": ""}, "symbol": "marker", "abbreviation": "ABCD"}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("abbreviation"), Some("condition_abbreviation")));
    let own = t.ok(
        m::TOOTH_CONDITIONS_SAVE,
        json!({"name": {"fa": "تغییر رنگ", "ps": "", "en": "Discoloration"}, "symbol": "marker", "abbreviation": "DC"}),
        Some(owner),
    );
    assert_eq!((own["is_system"].as_bool(), own["name"]["ps"].as_str()), (Some(false), Some("تغییر رنگ")));
    let actions = t.audit_actions(owner);
    for a in ["chart.finding_add", "chart.finding_remove", "chart.dentition", "tooth_condition.create"] {
        assert!(actions.contains(&a.to_string()), "{a}");
    }
}

#[test]
fn treatment_plans_alternatives_sessions_and_appointments() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    // The catalog says how each service is drawn and how far apart its sessions are.
    let catalog = t.ok(m::CATALOG_GET, json!({}), Some(owner));
    let svc = |code: &str| {
        catalog["services"].as_array().unwrap().iter().find(|s| s["code"] == code).unwrap().clone()
    };
    assert_eq!(svc("END-01D")["chart_symbol"], "root_canal");
    assert_eq!(svc("ORT-01M")["session_interval_days"], 30);
    let mut rct = svc("END-01D");
    rct["price"] = json!(350_000);
    t.ok(m::CATALOG_SAVE_SERVICE, rct.clone(), Some(owner));

    // Plan A from a ready-made plan on tooth 36; plan B as the alternative.
    let a = t.ok(
        m::PLANS_CREATE,
        json!({"patient_id": c.patient, "title": "گزینه الف", "template_id": plan_template_id(t, owner, "rct_crown"), "teeth": [36]}),
        Some(owner),
    );
    assert_eq!(a["status"], "proposed");
    let services: Vec<Value> =
        a["items"].as_array().unwrap().iter().map(|i| i["service_id"].clone()).collect();
    assert_eq!(
        services,
        vec![service_id(t, owner, "RAD-01"), service_id(t, owner, "END-01"), service_id(t, owner, "PRS-01")]
    );
    assert!(a["items"].as_array().unwrap().iter().all(|i| i["teeth"] == json!([36])));
    assert_eq!(a["items"][2]["phase"], 2);
    let b = t.ok(
        m::PLANS_CREATE,
        json!({"patient_id": c.patient, "title": "گزینه ب", "template_id": plan_template_id(t, owner, "extraction_implant"), "teeth": [36]}),
        Some(owner),
    );
    assert_eq!(b["items"].as_array().unwrap().len(), 5);
    let (_, field, rule) = rule_of(
        t,
        m::PLANS_CREATE,
        json!({"patient_id": c.patient, "title": "x", "template_id": plan_template_id(t, owner, "single_implant")}),
        owner,
    );
    assert_eq!(
        (field.as_deref(), rule.as_deref()),
        (Some("teeth"), Some("item_teeth")),
        "tooth lines need teeth"
    );

    // A one-tooth service on two teeth is two items at the catalog price; a filling needs surfaces.
    let items = t.ok(
        m::TREATMENT_ITEMS_ADD,
        json!({"patient_id": c.patient, "plan_id": a["id"], "service_id": rct["id"], "teeth": [37, 46]}),
        Some(owner),
    );
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        (items[0]["unit_price"].as_i64(), items[0]["sessions_planned"].as_i64()),
        (Some(350_000), Some(2))
    );
    let filling = service_id(t, owner, "RST-01C");
    let (_, field, rule) = rule_of(
        t,
        m::TREATMENT_ITEMS_ADD,
        json!({"patient_id": c.patient, "plan_id": a["id"], "service_id": filling, "teeth": [26]}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("surfaces"), Some("tooth_surface")));
    let bridge = t.ok(
        m::TREATMENT_ITEMS_ADD,
        json!({"patient_id": c.patient, "plan_id": a["id"], "service_id": service_id(t, owner, "PRS-02Z"),
               "teeth": [24, 25, 26], "unit_price": 900_000}),
        Some(owner),
    );
    assert_eq!((bridge[0]["units"].as_i64(), bridge[0]["unit_price"].as_i64()), (Some(3), Some(900_000)));
    let plans = t.ok(m::PLANS_LIST, json!({"patient_id": c.patient}), Some(owner));
    let total =
        plans.as_array().unwrap().iter().find(|p| p["id"] == a["id"]).unwrap()["total"].as_i64().unwrap();
    assert_eq!(total, 2 * 350_000 + 3 * 900_000, "unpriced catalog services count 0");

    // The patient chooses plan A: plan B is set aside with its items.
    let a = t.ok(
        m::PLANS_SET_STATUS,
        json!({"id": a["id"], "version": 1, "status": "accepted", "cancel_alternatives": true}),
        Some(owner),
    );
    assert_eq!(a["status"], "accepted");
    let plans = t.ok(m::PLANS_LIST, json!({"patient_id": c.patient}), Some(owner));
    let b = plans.as_array().unwrap().iter().find(|p| p["id"] == b["id"]).unwrap().clone();
    assert_eq!((b["status"].as_str(), b["superseded_by"].clone()), (Some("cancelled"), a["id"].clone()));
    assert!(b["items"].as_array().unwrap().iter().all(|i| i["status"] == "cancelled"));

    // Two visits booked as sessions 1 and 2 of the root canal on 37; a third is refused.
    let item = items[0].clone();
    let book = |start: &str, end: &str| {
        t.call(
            m::APPOINTMENTS_CREATE,
            json!({"patient_id": c.patient, "doctor_id": c.doctor, "date": c.today, "start_time": start, "end_time": end,
                   "treatment_item_id": item["id"]}),
            Some(owner),
        )
    };
    let v1 = book("09:00", "10:00").unwrap();
    assert_eq!(v1["treatment"]["session_no"], 1);
    assert_eq!(v1["treatment"]["sessions_planned"], 2);
    assert_eq!(v1["service_id"], rct["id"], "the visit is for the item's service");
    let v2 = book("15:00", "15:30").unwrap();
    assert_eq!(v2["treatment"]["session_no"], 2);
    let (_, field, rule) = rule_of(
        t,
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient, "doctor_id": c.doctor, "date": c.today, "start_time": "16:00", "end_time": "16:30",
               "treatment_item_id": item["id"]}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("session_no"), Some("session_number")));
    let (_, field, rule) = rule_of(
        t,
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": c.patient2, "doctor_id": c.doctor, "date": c.today, "start_time": "17:00", "end_time": "17:30",
               "treatment_item_id": item["id"]}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("treatment_item_id"), Some("item_patient")));

    // Completing the first visit records session 1: the item and the plan are in progress.
    let mut v = v1.clone();
    for status in ["checked_in", "in_treatment", "completed"] {
        v = t.ok(
            m::APPOINTMENTS_SET_STATUS,
            json!({"id": v["id"], "version": v["version"], "status": status}),
            Some(owner),
        );
    }
    assert_eq!(v["treatment"]["sessions_done"], 1);
    let plans = t.ok(m::PLANS_LIST, json!({"patient_id": c.patient}), Some(owner));
    let a = plans.as_array().unwrap().iter().find(|p| p["id"] == a["id"]).unwrap().clone();
    assert_eq!(a["status"], "in_progress");
    let rct37 = a["items"].as_array().unwrap().iter().find(|i| i["id"] == item["id"]).unwrap().clone();
    assert_eq!((rct37["status"].as_str(), rct37["sessions_done"].as_i64()), (Some("in_progress"), Some(1)));
    assert_eq!(rct37["upcoming"].as_array().unwrap().len(), 1, "session 2 is booked");
    assert_eq!(
        t.ok(m::TREATMENT_ITEMS_SUGGEST_NEXT, json!({"item_id": item["id"]}), Some(owner)),
        Value::Null
    );

    // The root canal on 46 has no booking: after its first session the next one is suggested a week later.
    let rct46 = items[1].clone();
    let r = t.ok(
        m::TREATMENT_ITEMS_RECORD_SESSION,
        json!({"item_id": rct46["id"], "version": rct46["version"], "doctor_id": c.doctor, "note": "دو کانال پاکسازی شد"}),
        Some(owner),
    );
    assert_eq!(
        (r["session"]["session_no"].as_i64(), r["item"]["status"].as_str()),
        (Some(1), Some("in_progress"))
    );
    assert_eq!(r["next"]["session_no"], 2);
    assert_eq!(r["next"]["interval_days"], 7);
    let today = crate::scheduling::parse_date("today", &c.today).unwrap();
    assert_eq!(r["next"]["date"], crate::scheduling::format_date(today + time::Duration::days(7)));
    let r2 = t.ok(
        m::TREATMENT_ITEMS_RECORD_SESSION,
        json!({"item_id": rct46["id"], "version": r["item"]["version"]}),
        Some(owner),
    );
    assert_eq!(r2["item"]["status"], "completed");
    assert_eq!(r2["next"], Value::Null);

    // Work in progress keeps the plan from being cancelled; the planned count cannot go below the done one.
    let (_, _, rule) = rule_of(
        t,
        m::PLANS_SET_STATUS,
        json!({"id": a["id"], "version": a["version"], "status": "cancelled"}),
        owner,
    );
    assert_eq!(rule.as_deref(), Some("plan_has_work_in_progress"));
    let (_, field, rule) = rule_of(
        t,
        m::TREATMENT_ITEMS_UPDATE,
        json!({"id": rct37["id"], "version": rct37["version"], "teeth": [37], "unit_price": 300_000, "phase": 1,
               "sessions_planned": 0}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("sessions_planned"), Some("item_sessions")));
    let changed = t.ok(
        m::TREATMENT_ITEMS_UPDATE,
        json!({"id": rct37["id"], "version": rct37["version"], "teeth": [37], "unit_price": 300_000, "phase": 1,
               "sessions_planned": 3, "details": "سه کانال"}),
        Some(owner),
    );
    assert_eq!(
        (changed["unit_price"].as_i64(), changed["sessions_planned"].as_i64()),
        (Some(300_000), Some(3))
    );

    // A treatment done today without a plan (5.8).
    let done = t.ok(
        m::TREATMENT_ITEMS_ADD,
        json!({"patient_id": c.patient, "service_id": service_id(t, owner, "SRG-01"), "teeth": [48], "perform_now": true,
               "doctor_id": c.doctor}),
        Some(owner),
    );
    assert_eq!((done[0]["status"].as_str(), done[0]["plan_id"].clone()), (Some("completed"), Value::Null));
    let treatments = t.ok(m::TREATMENTS_LIST, json!({"patient_id": c.patient}), Some(owner));
    assert!(treatments.as_array().unwrap().iter().any(|i| i["id"] == done[0]["id"]));
    assert!(treatments.as_array().unwrap().iter().any(|i| i["id"] == rct46["id"]));

    // Cancelling an item keeps its booked visit as an ordinary visit.
    let cancelled = t.ok(
        m::TREATMENT_ITEMS_SET_STATUS,
        json!({"id": changed["id"], "version": changed["version"], "status": "cancelled", "reason": "بیمار نخواست"}),
        Some(owner),
    );
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(t.ok(m::APPOINTMENTS_GET, json!({"id": v2["id"]}), Some(owner))["treatment"], Value::Null);
    let restored = t.ok(
        m::TREATMENT_ITEMS_SET_STATUS,
        json!({"id": cancelled["id"], "version": cancelled["version"], "status": "planned"}),
        Some(owner),
    );
    assert_eq!(restored["status"], "in_progress", "it had a session already");

    // The printed estimate (TP) freezes the plan as it was, in the document's language.
    let doc = t.ok(
        m::DOCUMENTS_ISSUE,
        json!({"patient_id": c.patient, "language": "en", "paper": "a4",
               "content": {"kind": "treatment_plan", "plan_ids": [a["id"]], "valid_days": 30}}),
        Some(owner),
    );
    assert!(doc["number"].as_str().unwrap().starts_with("TP-"));
    let snap = &doc["content"]["plans"][0];
    assert_eq!(snap["title"], "گزینه الف");
    assert!(snap["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["service"] == "Root canal treatment — Two canals"));
    assert_eq!(doc["content"]["numbering"], "quadrant");
    let (_, field, _) = rule_of(
        t,
        m::DOCUMENTS_ISSUE,
        json!({"patient_id": c.patient2, "language": "fa", "paper": "a4",
               "content": {"kind": "treatment_plan", "plan_ids": [a["id"]]}}),
        owner,
    );
    assert_eq!(field.as_deref(), Some("plan_ids"));

    // The timeline tells the story; reception (no clinical access) cannot read it but can book sessions.
    let timeline = t.ok(m::TIMELINE_LIST, json!({"patient_id": c.patient}), Some(owner));
    let actions = actions_of(&timeline);
    for a in [
        "patient.registered",
        "plan.created",
        "plan.accepted",
        "plan.superseded",
        "appointment.completed",
        "session.recorded",
        "item.completed",
        "document.issued",
    ] {
        assert!(actions.contains(&a.to_string()), "{a} in {actions:?}");
    }
    let only = t.ok(m::TIMELINE_LIST, json!({"patient_id": c.patient, "kinds": ["treatment"]}), Some(owner));
    assert!(only.as_array().unwrap().iter().all(|e| e["kind"] == "treatment"));
    t.ok(
        m::USERS_CREATE,
        json!({"username": "desk", "display_name": "پذیرش", "password": "desk-pass-1", "role": "receptionist"}),
        Some(owner),
    );
    let desk = t.login("desk", "desk-pass-1");
    assert_eq!(
        t.call(m::TIMELINE_LIST, json!({"patient_id": c.patient}), Some(&desk)),
        Err(ErrorCode::Forbidden)
    );
    assert_eq!(
        t.call(m::PLANS_LIST, json!({"patient_id": c.patient}), Some(&desk)),
        Err(ErrorCode::Forbidden)
    );
    let open = t.ok(m::TREATMENT_ITEMS_OPEN, json!({"patient_id": c.patient}), Some(&desk));
    assert!(!open.as_array().unwrap().is_empty());
    assert!(open
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["status"] == "planned" || i["status"] == "in_progress"));
}

#[test]
fn clinical_notes_lock_after_sign_off_and_take_addenda() {
    let Clinic4 { t, owner, patient, doctor, today, .. } = clinic4();
    let (_, field, rule) =
        rule_of(&t, m::NOTES_SAVE, json!({"patient_id": patient, "fields": {"diagnosis": "  "}}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("fields"), Some("note_empty")));
    let note = t.ok(
        m::NOTES_SAVE,
        json!({"patient_id": patient, "doctor_id": doctor, "fields": {"chief_complaint": "درد دندان ۳۶", "diagnosis": "پالپیت"}}),
        Some(&owner),
    );
    assert_eq!((note["status"].as_str(), note["visit_date"].as_str()), (Some("draft"), Some(today.as_str())));
    let note = t.ok(
        m::NOTES_SAVE,
        json!({"id": note["id"], "version": note["version"], "patient_id": patient, "doctor_id": doctor,
               "fields": {"chief_complaint": "درد دندان ۳۶", "diagnosis": "پالپیت برگشت‌ناپذیر", "procedure": "باز کردن حفره"}}),
        Some(&owner),
    );
    assert_eq!(note["fields"]["diagnosis"], "پالپیت برگشت‌ناپذیر");
    let signed = t.ok(m::NOTES_SIGN, json!({"id": note["id"], "version": note["version"]}), Some(&owner));
    assert_eq!(signed["status"], "signed");
    assert!(signed["signed_at"].is_string());
    let (_, _, rule) = rule_of(
        &t,
        m::NOTES_SAVE,
        json!({"id": signed["id"], "version": signed["version"], "patient_id": patient, "fields": {"diagnosis": "x"}}),
        &owner,
    );
    assert_eq!(rule.as_deref(), Some("note_signed"));
    let (_, _, rule) =
        rule_of(&t, m::NOTES_DELETE, json!({"id": signed["id"], "version": signed["version"]}), &owner);
    assert_eq!(rule.as_deref(), Some("note_signed"));
    let with = t.ok(
        m::NOTES_ADD_ADDENDUM,
        json!({"note_id": signed["id"], "text": "عکس پری‌اپیکال ضایعه را نشان داد."}),
        Some(&owner),
    );
    assert_eq!(with["addenda"].as_array().unwrap().len(), 1);
    assert_eq!(with["fields"]["diagnosis"], "پالپیت برگشت‌ناپذیر", "the signed text never changes");
    // A note saved and signed in one step; a draft thrown away.
    let quick = t.ok(
        m::NOTES_SAVE,
        json!({"patient_id": patient, "fields": {"procedure": "جرم‌گیری"}, "sign": true}),
        Some(&owner),
    );
    assert_eq!(quick["status"], "signed");
    let draft =
        t.ok(m::NOTES_SAVE, json!({"patient_id": patient, "fields": {"notes": "پیش‌نویس"}}), Some(&owner));
    t.ok(m::NOTES_DELETE, json!({"id": draft["id"], "version": draft["version"]}), Some(&owner));
    assert_eq!(
        t.ok(m::NOTES_LIST, json!({"patient_id": patient}), Some(&owner)).as_array().unwrap().len(),
        2
    );
    let exams =
        t.ok(m::TIMELINE_LIST, json!({"patient_id": patient, "kinds": ["examination"]}), Some(&owner));
    assert_eq!(exams.as_array().unwrap().len(), 2);

    // Ready-made notes are the clinic's to rewrite, survive a restart, and can be put back.
    let templates = t.ok(m::NOTE_TEMPLATES_LIST, json!({}), Some(&owner));
    let rct = templates.as_array().unwrap().iter().find(|x| x["code"] == "rct_first").unwrap().clone();
    let mut edit = rct.clone();
    edit["body"]["fa"]["procedure"] = json!("متن کلینیک");
    let edited = t.ok(m::NOTE_TEMPLATES_SAVE, edit, Some(&owner));
    assert_eq!(
        (edited["customized"].as_bool(), edited["body"]["fa"]["procedure"].as_str()),
        (Some(true), Some("متن کلینیک"))
    );
    let t = t.reopen();
    let owner = t.login("owner", "owner-pass-1");
    let kept = t.ok(m::NOTE_TEMPLATES_LIST, json!({}), Some(&owner));
    let kept = kept.as_array().unwrap().iter().find(|x| x["code"] == "rct_first").unwrap().clone();
    assert_eq!(kept["body"]["fa"]["procedure"], "متن کلینیک", "a restart never overwrites the clinic's text");
    let reset =
        t.ok(m::NOTE_TEMPLATES_RESET, json!({"id": kept["id"], "version": kept["version"]}), Some(&owner));
    assert_eq!(reset["customized"], false);
    assert_ne!(reset["body"]["fa"]["procedure"], "متن کلینیک");
}

#[test]
fn perio_exams_are_checked_summarised_and_listed_newest_first() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    let tooth = |n: u8, d: [u8; 6], bleed: [bool; 6]| {
        json!({"tooth": n, "depth": d, "recession": [0, 1, 0, 0, 0, 0], "bleeding": bleed, "plaque": [false, false, false, false, false, false],
               "mobility": 1, "furcation": [null, null]})
    };
    let (_, field, rule) = rule_of(
        t,
        m::PERIO_SAVE,
        json!({"patient_id": c.patient, "teeth": [tooth(16, [3, 2, 20, 3, 2, 3], [false; 6])]}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("teeth.0.depth"), Some("perio_value")));
    let gone = json!({"tooth": 18, "missing": true, "depth": [9, 9, 9, 9, 9, 9], "recession": [null, null, null, null, null, null],
                      "bleeding": [true, true, true, true, true, true], "plaque": [false, false, false, false, false, false], "mobility": null, "furcation": [null, null]});
    let first = t.ok(
        m::PERIO_SAVE,
        json!({"patient_id": c.patient, "exam_date": "2026-01-10",
               "teeth": [tooth(16, [5, 4, 6, 4, 3, 5], [true, true, false, true, false, false]), tooth(26, [3; 6], [false; 6]), gone]}),
        Some(owner),
    );
    let s = &first["summary"];
    assert_eq!(
        (s["teeth"].as_i64(), s["sites_4_plus"].as_i64(), s["sites_6_plus"].as_i64()),
        (Some(2), Some(5), Some(1))
    );
    assert_eq!(s["bleeding_percent"], 25, "3 of 12 sites; the missing tooth is not counted");
    let second = t.ok(
        m::PERIO_SAVE,
        json!({"patient_id": c.patient, "teeth": [tooth(16, [3, 3, 4, 3, 2, 3], [false; 6]), tooth(26, [2; 6], [false; 6])]}),
        Some(owner),
    );
    let list = t.ok(m::PERIO_LIST, json!({"patient_id": c.patient}), Some(owner));
    assert_eq!(list[0]["id"], second["id"], "newest first");
    assert!(list[0]["summary"]["mean_depth_x10"].as_i64() < list[1]["summary"]["mean_depth_x10"].as_i64());
    t.ok(m::PERIO_DELETE, json!({"id": first["id"], "version": first["version"]}), Some(owner));
    assert_eq!(
        t.ok(m::PERIO_LIST, json!({"patient_id": c.patient}), Some(owner)).as_array().unwrap().len(),
        1
    );
}

#[test]
fn migration_to_the_clinical_core_keeps_documents_and_draws_the_old_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::new(Environment::Test, dir.path().to_path_buf());
    let recovery = RecoveryKey::generate();
    let protector = default_protector();
    let path = config.db_path();
    let (cat, svc) =
        (crate::ids::seed_id("service_category", "END"), crate::ids::seed_id("service", "END-01"));
    {
        let (mut conn, _key) = db::create(&path, protector.as_ref(), &recovery).unwrap();
        let v12: Vec<db::Migration> = db::MIGRATIONS
            .iter()
            .filter(|m| m.version <= 12)
            .map(|m| db::Migration { version: m.version, name: m.name, sql: m.sql })
            .collect();
        db::migrate(&mut conn, &v12, &config.backup_dir()).unwrap();
        conn.execute_batch(&format!(
            "INSERT INTO db_meta(key, value) VALUES ('clinic_name', 'Old'), ('default_language', 'fa'), ('clinic_id', 'x'),
                    ('created_at', '2026-01-01T00:00:00.000Z'), ('created_with_version', '0.5.0');
             INSERT INTO patient(id, patient_number, full_name, registration_date, created_at, updated_at)
             VALUES ('p1', 'P-000001', 'Old Patient', '2026-01-01', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');
             INSERT INTO clinical_document(id, number, kind, patient_id, language, paper, patient_json, content_json, issued_at,
                 created_at, updated_at)
             VALUES ('d1', 'RX-1405-000001', 'prescription', 'p1', 'fa', 'a5', '{{}}', '{{}}', '2026-01-02T00:00:00.000Z',
                 '2026-01-02T00:00:00.000Z', '2026-01-02T00:00:00.000Z');
             INSERT INTO service_category(id, code, name_fa, name_ps, name_en, is_system, created_at, updated_at)
             VALUES ('{cat}', 'END', 'اندو', 'اندو', 'Endodontics', 1, '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');
             INSERT INTO service(id, code, category_id, name_fa, name_ps, name_en, price, tooth_scope, sessions, is_system,
                 created_at, updated_at)
             VALUES ('{svc}', 'END-01', '{cat}', 'عصب‌کشی', 'عصب ایستل', 'Root canal treatment', 250000, 'tooth', 2, 1,
                 '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');"
        ))
        .unwrap();
    }
    let core = Core::open(config).unwrap();
    let (number, symbol, price): (String, String, i64) = core
        .with_db(|o| {
            Ok(o.conn.query_row(
                "SELECT d.number, s.chart_symbol, s.price FROM clinical_document d, service s
                 WHERE d.id = 'd1' AND s.code = 'END-01'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .unwrap();
    assert_eq!(number, "RX-1405-000001", "issued documents survive the table rebuild");
    assert_eq!(symbol, "root_canal", "the built-in catalog learns how it is drawn");
    assert_eq!(price, 250000, "the clinic's price is untouched");
    let widened: i64 = core
        .with_db(|o| {
            Ok(o.conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'clinical_document' AND sql LIKE '%treatment_plan%'",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(widened, 1, "estimates (TP) can be stored");
}

#[test]
fn the_record_summary_prints_the_clinical_visit() {
    // Phase 5 exit: a whole clinical visit — chart, treatment, note — is in the printed record.
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    t.ok(
        m::CHART_ADD_FINDINGS,
        json!({"patient_id": c.patient, "condition_id": condition_id(t, owner, "caries"), "teeth": [36], "surfaces": "OD"}),
        Some(owner),
    );
    t.ok(
        m::CHART_ADD_FINDINGS,
        json!({"patient_id": c.patient, "condition_id": condition_id(t, owner, "missing"), "teeth": [18]}),
        Some(owner),
    );
    let item = &t.ok(
        m::TREATMENT_ITEMS_ADD,
        json!({"patient_id": c.patient, "service_id": service_id(t, owner, "END-01D"), "teeth": [36], "doctor_id": c.doctor}),
        Some(owner),
    )[0];
    t.ok(
        m::TREATMENT_ITEMS_RECORD_SESSION,
        json!({"item_id": item["id"], "version": item["version"], "doctor_id": c.doctor}),
        Some(owner),
    );
    t.ok(
        m::NOTES_SAVE,
        json!({"patient_id": c.patient, "doctor_id": c.doctor, "fields": {"diagnosis": "پالپیت", "procedure": "پاکسازی کانال"}, "sign": true}),
        Some(owner),
    );
    t.ok(m::NOTES_SAVE, json!({"patient_id": c.patient, "fields": {"notes": "پیش‌نویس"}}), Some(owner));
    let doc = t.ok(
        m::DOCUMENTS_ISSUE,
        json!({"patient_id": c.patient, "language": "en", "paper": "a4", "content": {"kind": "record_summary"}}),
        Some(owner),
    );
    let snap = &doc["content"]["snapshot"];
    assert_eq!(snap["numbering"], "quadrant");
    assert_eq!(snap["findings"][0], json!({"tooth": 18, "condition": "Missing", "surfaces": ""}), "by tooth");
    assert_eq!(snap["findings"][1]["surfaces"], "OD");
    assert_eq!(snap["treatments"].as_array().unwrap().len(), 1);
    let rct = &snap["treatments"][0];
    assert_eq!(rct["service"], "Root canal treatment — Two canals", "in the document's language");
    assert_eq!((rct["sessions_done"].as_i64(), rct["sessions_planned"].as_i64()), (Some(1), Some(2)));
    assert_eq!((rct["status"].as_str(), rct["doctor"].as_str()), (Some("in_progress"), Some("دکتر احمد")));
    assert_eq!(rct["date"], c.today);
    // Only signed notes are printed.
    assert_eq!(snap["notes"].as_array().unwrap().len(), 1);
    assert_eq!(snap["notes"][0]["procedure"], "پاکسازی کانال");
    // A summary issued before v0.6.0 (no clinical part) still opens.
    let old: artaveo_shared::RecordSnapshot = serde_json::from_value(
        json!({"medical": [], "medical_notes": null, "visits": [], "prescriptions": []}),
    )
    .unwrap();
    assert!(old.findings.is_empty() && old.numbering == artaveo_shared::ToothNumbering::Fdi);
}

// ───────────────────────────── Phase 6: billing ─────────────────────────────

fn method_id(t: &T, token: &str, code: &str) -> Value {
    t.ok(m::PAYMENT_METHODS_LIST, json!({}), Some(token))
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["code"] == code)
        .unwrap_or_else(|| panic!("payment method {code}"))["id"]
        .clone()
}

/// A treatment done today at the given price (AFN × 100).
fn treatment_done(c: &Clinic4, code: &str, tooth: u8, price: i64) -> Value {
    let owner = c.owner.as_str();
    let item = c.t.ok(
        m::TREATMENT_ITEMS_ADD,
        json!({"patient_id": c.patient, "service_id": service_id(&c.t, owner, code), "teeth": [tooth],
               "perform_now": true, "doctor_id": c.doctor, "unit_price": price}),
        Some(owner),
    );
    item[0].clone()
}

#[test]
fn billing_cycle_invoice_payments_installments_refund_and_cash_closing() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    let cash = method_id(t, owner, "cash");
    let bank = method_id(t, owner, "bank_transfer");
    let root = treatment_done(&c, "SRG-01", 36, 350_000);
    let filling = treatment_done(&c, "SRG-01", 48, 80_000);

    // Treatments done are ready for an invoice.
    let ready = t.ok(m::INVOICES_BILLABLE, json!({"patient_id": c.patient}), Some(owner));
    assert_eq!(ready.as_array().unwrap().len(), 2);
    let account = t.ok(m::BILLING_ACCOUNT, json!({"patient_id": c.patient}), Some(owner));
    assert_eq!(
        (account["unbilled_count"].as_i64(), account["unbilled_total"].as_i64()),
        (Some(2), Some(430_000))
    );

    // An empty invoice, a discount above the total, and a stranger's treatment are refused.
    let (_, field, rule) = rule_of(t, m::INVOICES_CREATE, json!({"patient_id": c.patient}), owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("items"), Some("invoice_empty")));
    let (_, field, rule) = rule_of(
        t,
        m::INVOICES_CREATE,
        json!({"patient_id": c.patient, "items": [{"item_id": root["id"]}], "discount": 400_000}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("discount"), Some("discount_range")));
    let (_, field, rule) = rule_of(
        t,
        m::INVOICES_CREATE,
        json!({"patient_id": c.patient2, "items": [{"item_id": root["id"]}]}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("items"), Some("item_not_billable")));

    // The root canal with a discount.
    let inv = t.ok(
        m::INVOICES_CREATE,
        json!({"patient_id": c.patient, "items": [{"item_id": root["id"]}], "discount": 50_000, "note": "اولین فاکتور"}),
        Some(owner),
    );
    assert!(inv["number"].as_str().unwrap().starts_with("INV-"), "{inv}");
    assert!(inv["number"].as_str().unwrap().ends_with("-000001"));
    assert_eq!(
        (inv["subtotal"].as_i64(), inv["net"].as_i64(), inv["remaining"].as_i64()),
        (Some(350_000), Some(300_000), Some(300_000))
    );
    assert_eq!(inv["pay_state"], "unpaid");
    let treatments = t.ok(m::TREATMENTS_LIST, json!({"patient_id": c.patient}), Some(owner));
    let rc = treatments.as_array().unwrap().iter().find(|i| i["id"] == root["id"]).unwrap().clone();
    assert_eq!(rc["billed"], true);
    let (_, field, rule) = rule_of(
        t,
        m::INVOICES_CREATE,
        json!({"patient_id": c.patient, "items": [{"item_id": root["id"]}]}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("items"), Some("item_already_billed")));

    // Reception can invoice at the treatment's price, not at another one (6.1), and cannot void.
    t.ok(
        m::USERS_CREATE,
        json!({"username": "reception", "display_name": "پذیرش", "password": "recep-pass-1", "role": "receptionist"}),
        Some(owner),
    );
    let rec = t.login("reception", "recep-pass-1");
    assert_eq!(
        t.call(
            m::INVOICES_CREATE,
            json!({"patient_id": c.patient, "items": [{"item_id": filling["id"], "unit_price": 10_000}]}),
            Some(&rec)
        ),
        Err(ErrorCode::Forbidden)
    );
    assert_eq!(
        t.call(
            m::INVOICES_VOID,
            json!({"id": inv["id"], "version": inv["version"], "reason": "x"}),
            Some(&rec)
        ),
        Err(ErrorCode::Forbidden),
        "voiding needs billing.void"
    );
    assert!(t.call(m::CASH_DAY, json!({"date": c.today}), Some(&rec)).is_ok());

    // A partial payment in cash: the receipt says what is left.
    let (_, field, rule) = rule_of(
        t,
        m::PAYMENTS_ADD,
        json!({"patient_id": c.patient, "invoice_id": inv["id"], "amount": 300_001, "method_id": cash}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("amount"), Some("payment_exceeds")));
    let (_, field, _) = rule_of(
        t,
        m::PAYMENTS_ADD,
        json!({"patient_id": c.patient, "invoice_id": inv["id"], "amount": 100_000}),
        owner,
    );
    assert_eq!(field.as_deref(), Some("method_id"));
    let p1 = t.ok(
        m::PAYMENTS_ADD,
        json!({"patient_id": c.patient, "invoice_id": inv["id"], "amount": 100_000, "method_id": cash}),
        Some(owner),
    );
    assert!(p1["number"].as_str().unwrap().starts_with("RC-"));
    assert_eq!(
        (
            p1["receipt"]["remaining_after"].as_i64(),
            p1["receipt"]["paid_after"].as_i64(),
            p1["receipt"]["lines"].as_array().unwrap().len()
        ),
        (Some(200_000), Some(100_000), 1)
    );

    // Installments for the rest: they must add up, and one is already late.
    let yesterday = plus_days(&c.today, -1);
    let in_a_month = plus_days(&c.today, 30);
    let (_, field, rule) = rule_of(
        t,
        m::INVOICES_SET_INSTALLMENTS,
        json!({"invoice_id": inv["id"], "installments": [{"due_date": yesterday, "amount": 50_000}]}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("installments"), Some("installments_invalid")));
    let with_plan = t.ok(
        m::INVOICES_SET_INSTALLMENTS,
        json!({"invoice_id": inv["id"], "installments": [
            {"due_date": in_a_month, "amount": 100_000}, {"due_date": yesterday, "amount": 100_000}]}),
        Some(owner),
    );
    let plan = with_plan["installments"].as_array().unwrap();
    assert_eq!(
        (plan[0]["due_date"].as_str(), plan[0]["state"].as_str()),
        (Some(yesterday.as_str()), Some("overdue"))
    );
    let due = t.ok(m::BILLING_INSTALLMENTS_DUE, json!({}), Some(owner));
    assert_eq!(due.as_array().unwrap().len(), 1, "only the late one is due this week");
    assert_eq!((due[0]["overdue"].as_bool(), due[0]["unpaid"].as_i64()), (Some(true), Some(100_000)));
    let debtors = t.ok(m::BILLING_DEBTORS, json!({}), Some(owner));
    assert_eq!(
        (debtors[0]["remaining"].as_i64(), debtors[0]["overdue"].as_bool()),
        (Some(200_000), Some(true))
    );

    // Paying the late installment by bank transfer settles it; the other stays.
    let p2 = t.ok(
        m::PAYMENTS_ADD,
        json!({"patient_id": c.patient, "invoice_id": inv["id"], "amount": 100_000, "method_id": bank}),
        Some(owner),
    );
    assert_eq!(p2["receipt"]["remaining_after"], 100_000);
    let due = t.ok(m::BILLING_INSTALLMENTS_DUE, json!({"through": in_a_month}), Some(owner));
    assert_eq!(due.as_array().unwrap().len(), 1);
    assert_eq!(due[0]["no"], 2);

    // An advance payment becomes credit, which pays the invoice without moving money.
    let adv = t.ok(
        m::PAYMENTS_ADD,
        json!({"patient_id": c.patient, "amount": 50_000, "method_id": cash, "note": "پیش‌پرداخت"}),
        Some(owner),
    );
    assert_eq!(
        (adv["invoice_id"].clone(), adv["receipt"]["credit_after"].as_i64()),
        (Value::Null, Some(50_000))
    );
    let (_, field, rule) = rule_of(
        t,
        m::PAYMENTS_ADD,
        json!({"patient_id": c.patient, "invoice_id": inv["id"], "amount": 60_000, "use_credit": true}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("amount"), Some("insufficient_credit")));
    let used = t.ok(
        m::PAYMENTS_ADD,
        json!({"patient_id": c.patient, "invoice_id": inv["id"], "amount": 50_000, "use_credit": true}),
        Some(owner),
    );
    assert!(used["number"].as_str().unwrap().starts_with("CU-"));
    let account = t.ok(m::BILLING_ACCOUNT, json!({"patient_id": c.patient}), Some(owner));
    assert_eq!(
        (
            account["paid"].as_i64(),
            account["remaining"].as_i64(),
            account["credit"].as_i64(),
            account["discount"].as_i64()
        ),
        (Some(250_000), Some(50_000), Some(0), Some(50_000))
    );

    // A refund of part of the first cash payment: the invoice owes that much again.
    let (_, field, rule) = rule_of(
        t,
        m::PAYMENTS_REFUND,
        json!({"payment_id": p1["id"], "amount": 100_001, "reason": "x"}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("amount"), Some("refund_exceeds")));
    assert_eq!(
        t.call(
            m::PAYMENTS_REFUND,
            json!({"payment_id": p1["id"], "amount": 20_000, "reason": "x"}),
            Some(&rec)
        ),
        Err(ErrorCode::Forbidden)
    );
    let rv = t.ok(
        m::PAYMENTS_REFUND,
        json!({"payment_id": p1["id"], "amount": 20_000, "reason": "اشتباه"}),
        Some(owner),
    );
    assert!(rv["number"].as_str().unwrap().starts_with("RV-"));
    assert_eq!(rv["receipt"]["remaining_after"], 70_000);
    let again = t.ok(m::PAYMENTS_GET, json!({"id": p1["id"]}), Some(owner));
    assert_eq!(again["refunded"], 20_000);

    // The invoice cannot be voided while it holds money; a credit note lowers it instead.
    let inv_now = t.ok(m::INVOICES_GET, json!({"id": inv["id"]}), Some(owner));
    let (_, _, rule) = rule_of(
        t,
        m::INVOICES_VOID,
        json!({"id": inv_now["id"], "version": inv_now["version"], "reason": "اشتباه"}),
        owner,
    );
    assert_eq!(rule.as_deref(), Some("invoice_has_payments"));
    let (_, field, rule) = rule_of(
        t,
        m::INVOICES_CREDIT_NOTE,
        json!({"invoice_id": inv["id"], "amount": 80_000, "reason": "x"}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("amount"), Some("credit_note_exceeds")));
    let noted = t.ok(
        m::INVOICES_CREDIT_NOTE,
        json!({"invoice_id": inv["id"], "amount": 10_000, "reason": "تخفیف بعدی"}),
        Some(owner),
    );
    assert_eq!((noted["net"].as_i64(), noted["remaining"].as_i64()), (Some(290_000), Some(60_000)));
    assert!(noted["credit_notes"][0]["number"].as_str().unwrap().starts_with("CN-"));

    // The statement ends at what the patient owes, and says the same as the account.
    let st = t.ok(m::BILLING_STATEMENT, json!({"patient_id": c.patient}), Some(owner));
    let last = st["lines"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["balance"], 60_000, "{st}");
    assert_eq!(st["account"]["remaining"], 60_000);

    // A second invoice voided frees its treatment again.
    let inv2 = t.ok(
        m::INVOICES_CREATE,
        json!({"patient_id": c.patient, "items": [{"item_id": filling["id"]}]}),
        Some(owner),
    );
    t.ok(
        m::INVOICES_VOID,
        json!({"id": inv2["id"], "version": inv2["version"], "reason": "اشتباه"}),
        Some(owner),
    );
    let ready = t.ok(m::INVOICES_BILLABLE, json!({"patient_id": c.patient}), Some(owner));
    assert_eq!(ready.as_array().unwrap().len(), 1);
    assert_eq!(t.ok(m::INVOICES_GET, json!({"id": inv2["id"]}), Some(owner))["pay_state"], "void");

    // A reprint is a count, never a new payment.
    let before =
        t.ok(m::PAYMENTS_LIST, json!({"patient_id": c.patient}), Some(owner)).as_array().unwrap().len();
    let printed = t.ok(m::PAYMENTS_MARK_PRINTED, json!({"id": p1["id"], "pdf": false}), Some(owner));
    assert_eq!(printed["print_count"], 1);
    assert_eq!(
        t.ok(m::PAYMENTS_LIST, json!({"patient_id": c.patient}), Some(owner)).as_array().unwrap().len(),
        before
    );

    // Cash closing: in 100 000 + 50 000 cash, 20 000 cash back, 10 000 spent on materials.
    let cat = t.ok(m::EXPENSE_CATEGORIES_LIST, json!({}), Some(owner))[0]["id"].clone();
    t.ok(
        m::EXPENSES_ADD,
        json!({"category_id": cat, "amount": 10_000, "date": c.today, "description": "ماسک و دستکش"}),
        Some(owner),
    );
    let day = t.ok(m::CASH_DAY, json!({"date": c.today}), Some(owner));
    assert_eq!(
        (
            day["cash_in"].as_i64(),
            day["cash_refunds"].as_i64(),
            day["cash_expenses"].as_i64(),
            day["expected"].as_i64()
        ),
        (Some(150_000), Some(20_000), Some(10_000), Some(120_000))
    );
    assert_eq!(day["by_method"].as_array().unwrap().len(), 2, "cash and bank");
    let (_, field, rule) = rule_of(t, m::CASH_CLOSE, json!({"date": c.today, "counted": 119_000}), owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("reason"), Some("cash_reason_required")));
    let closing =
        t.ok(m::CASH_CLOSE, json!({"date": c.today, "counted": 119_000, "reason": "خرد نشد"}), Some(owner));
    assert_eq!((closing["expected"].as_i64(), closing["difference"].as_i64()), (Some(120_000), Some(-1_000)));
    let (_, _, rule) = rule_of(t, m::CASH_CLOSE, json!({"date": c.today, "counted": 120_000}), owner);
    assert_eq!(rule.as_deref(), Some("cash_day_closed"));
    let (_, _, rule) = rule_of(
        t,
        m::EXPENSES_ADD,
        json!({"category_id": cat, "amount": 1_000, "date": c.today, "description": "بعد از بستن"}),
        owner,
    );
    assert_eq!(rule.as_deref(), Some("cash_day_closed"));

    // Profit summary of the day.
    let sum = t.ok(m::BILLING_SUMMARY, json!({"from": c.today, "to": c.today}), Some(owner));
    assert_eq!(
        (
            sum["received"].as_i64(),
            sum["refunded"].as_i64(),
            sum["expenses"].as_i64(),
            sum["profit"].as_i64()
        ),
        (Some(250_000), Some(20_000), Some(10_000), Some(220_000))
    );

    // Commission: 20 % of what was received, shared by the invoice's lines; off by default.
    let off = t.ok(m::COMMISSION_REPORT, json!({"from": c.today, "to": c.today}), Some(owner));
    assert!(off["doctors"].as_array().unwrap().is_empty());
    t.ok(
        m::BILLING_SETTINGS_SAVE,
        json!({"commission_enabled": true, "commission_basis": "payment", "receipt_footer": "با تشکر"}),
        Some(owner),
    );
    t.ok(m::COMMISSION_RULES_SAVE, json!({"kind": "percent", "value": 2000}), Some(owner));
    let report = t.ok(m::COMMISSION_REPORT, json!({"from": c.today, "to": c.today}), Some(owner));
    // Received for the invoice: 100 000 + 100 000 + 50 000 credit − 20 000 refund = 230 000 of a net of 290 000.
    assert_eq!(report["doctors"][0]["doctor_id"], c.doctor.as_str());
    assert_eq!(report["doctors"][0]["base"], 230_000);
    assert_eq!(report["doctors"][0]["commission"], 46_000);
    t.ok(
        m::BILLING_SETTINGS_SAVE,
        json!({"commission_enabled": true, "commission_basis": "treatment", "receipt_footer": ""}),
        Some(owner),
    );
    let report = t.ok(m::COMMISSION_REPORT, json!({"from": c.today, "to": c.today}), Some(owner));
    assert_eq!(report["doctors"][0]["base"], 290_000, "the net of the invoiced treatment");
    assert_eq!(report["doctors"][0]["commission"], 58_000);

    let actions = t.audit_actions(owner);
    for a in [
        "invoice.create",
        "invoice.void",
        "invoice.credit_note",
        "invoice.installments",
        "payment.add",
        "payment.credit",
        "payment.refund",
        "payment.print",
        "expense.add",
        "cash.close",
        "billing.settings",
        "commission.rule",
    ] {
        assert!(actions.contains(&a.to_string()), "{a}");
    }
}

#[test]
fn payments_after_a_patient_merge_follow_the_survivor() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    let cash = method_id(t, owner, "cash");
    let item = treatment_done(&c, "SRG-01", 48, 80_000);
    let inv = t.ok(
        m::INVOICES_CREATE,
        json!({"patient_id": c.patient, "items": [{"item_id": item["id"]}]}),
        Some(owner),
    );
    t.ok(
        m::PAYMENTS_ADD,
        json!({"patient_id": c.patient, "invoice_id": inv["id"], "amount": 30_000, "method_id": cash}),
        Some(owner),
    );
    let loser = t.ok(m::PATIENTS_GET, json!({"patient_id": c.patient}), Some(owner));
    let keeper = t.ok(m::PATIENTS_GET, json!({"patient_id": c.patient2}), Some(owner));
    t.ok(
        m::PATIENTS_MERGE,
        json!({"keep_id": keeper["id"], "merge_id": loser["id"], "merge_id_version": loser["version"]}),
        Some(owner),
    );
    let account = t.ok(m::BILLING_ACCOUNT, json!({"patient_id": c.patient2}), Some(owner));
    assert_eq!((account["invoice_count"].as_i64(), account["remaining"].as_i64()), (Some(1), Some(50_000)));
    let debtors = t.ok(m::BILLING_DEBTORS, json!({}), Some(owner));
    assert_eq!(debtors[0]["patient_id"], keeper["id"]);
}

// ───────────────────────────── Phase 8: inventory, suppliers, dental lab ─────────────────────────────

/// A clinic date `n` days from today (negative = the past).
fn day(n: i64) -> String {
    crate::scheduling::format_date(crate::clock::local_now().date() + time::Duration::days(n))
}

fn first_id(t: &T, method: &str, owner: &str, index: usize) -> String {
    s_of(&t.ok(method, json!({}), Some(owner))[index], "id")
}

#[test]
fn inventory_stock_is_taken_first_expiry_first_and_expired_is_never_used() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let cat = first_id(&t, m::STOCK_CATEGORIES_LIST, &owner, 0);
    let unit = first_id(&t, m::STOCK_UNITS_LIST, &owner, 0);
    let main = first_id(&t, m::STOCK_LOCATIONS_LIST, &owner, 0);
    let room = first_id(&t, m::STOCK_LOCATIONS_LIST, &owner, 1);
    let item = t.ok(
        m::STOCK_ITEMS_SAVE,
        json!({"name": {"fa": "دستکش", "ps": "", "en": "Gloves"}, "category_id": cat, "unit_id": unit, "min_qty": 500}),
        Some(&owner),
    );
    assert_eq!(item["name"]["ps"], "دستکش", "an empty language is filled from the first one");
    let id = s_of(&item, "id");
    let receive = |qty: i64, cost: i64, lot: &str, expiry: String, place: &str| {
        t.ok(
            m::STOCK_RECEIVE,
            json!({"item_id": id, "qty": qty, "unit_cost": cost, "lot_no": lot, "expiry_date": expiry, "location_id": place}),
            Some(&owner),
        )
    };
    receive(1000, 100, "A", day(-5), &main);
    receive(400, 200, "B", day(30), &main);
    let after = receive(300, 300, "C", day(200), &main);
    assert_eq!(
        (
            after["qty"].as_i64(),
            after["usable_qty"].as_i64(),
            after["expired_qty"].as_i64(),
            after["expiring_qty"].as_i64()
        ),
        (Some(1700), Some(700), Some(1000), Some(400)),
        "{after}"
    );
    assert_eq!(after["low"], false);
    // 10.00 × 1.00 + 4.00 × 2.00 + 3.00 × 3.00 = 27.00 AFN, in AFN × 100.
    assert_eq!(after["value"], 2_700);

    // 5.00 units: the batch that expires first (B, 4.00), then C; the expired batch is untouched.
    let used = t.ok(m::STOCK_CONSUME, json!({"item_id": id, "qty": 500}), Some(&owner));
    assert_eq!((used["usable_qty"].as_i64(), used["expired_qty"].as_i64()), (Some(200), Some(1000)));
    assert_eq!(used["low"], true, "200 is under the minimum of 500");
    let batches = t.ok(m::STOCK_BATCHES_LIST, json!({"item_id": id}), Some(&owner));
    let lots: Vec<_> = batches.as_array().unwrap().iter().map(|b| b["lot_no"].as_str().unwrap()).collect();
    assert_eq!(lots, vec!["A", "C"], "B is empty and hidden; the expired one is listed first");
    assert_eq!(batches[0]["state"], "expired");

    let (_, field, rule) = rule_of(&t, m::STOCK_CONSUME, json!({"item_id": id, "qty": 300}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("qty"), Some("stock_insufficient")));
    let (_, field, rule) = rule_of(&t, m::STOCK_CONSUME, json!({"item_id": id, "qty": 0}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("qty"), Some("stock_quantity")));

    // Throwing the expired lot away needs a reason; stock can never go below zero.
    let expired = s_of(&batches[0], "id");
    let (_, field, rule) =
        rule_of(&t, m::STOCK_ADJUST, json!({"batch_id": expired, "delta": -1000, "reason": " "}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("reason"), Some("required")));
    let (_, _, rule) =
        rule_of(&t, m::STOCK_ADJUST, json!({"batch_id": expired, "delta": -1001, "reason": "x"}), &owner);
    assert_eq!(rule.as_deref(), Some("stock_insufficient"));
    let thrown =
        t.ok(m::STOCK_ADJUST, json!({"batch_id": expired, "delta": -1000, "reason": "منقضی"}), Some(&owner));
    assert_eq!(thrown["expired_qty"], 0);

    // Moving some of C to the treatment room splits the batch; moving to the same place is refused.
    let c_batch = s_of(&t.ok(m::STOCK_BATCHES_LIST, json!({"item_id": id}), Some(&owner))[0], "id");
    t.ok(m::STOCK_TRANSFER, json!({"batch_id": c_batch, "qty": 100, "to_location_id": room}), Some(&owner));
    let (_, _, rule) = rule_of(
        &t,
        m::STOCK_TRANSFER,
        json!({"batch_id": c_batch, "qty": 10, "to_location_id": main}),
        &owner,
    );
    assert_eq!(rule.as_deref(), Some("stock_same_place"));
    let split = t.ok(m::STOCK_BATCHES_LIST, json!({"item_id": id}), Some(&owner));
    assert_eq!(split.as_array().unwrap().len(), 2);
    assert_eq!(
        t.ok(m::STOCK_ITEMS_GET, json!({"id": id}), Some(&owner))["usable_qty"],
        200,
        "a transfer keeps the total"
    );

    // Taking from one place only.
    let (_, _, rule) =
        rule_of(&t, m::STOCK_CONSUME, json!({"item_id": id, "qty": 150, "location_id": room}), &owner);
    assert_eq!(rule.as_deref(), Some("stock_insufficient"));
    t.ok(m::STOCK_CONSUME, json!({"item_id": id, "qty": 100, "location_id": room}), Some(&owner));

    // Alerts and filters.
    let alerts = t.ok(m::STOCK_ALERTS_LIST, json!({}), Some(&owner));
    assert_eq!(alerts["low"].as_array().unwrap().len(), 1);
    assert!(alerts["expired"].as_array().unwrap().is_empty());
    let low = t.ok(m::STOCK_ITEMS_LIST, json!({"only": "low"}), Some(&owner));
    assert_eq!(low.as_array().unwrap().len(), 1);
    let none = t.ok(m::STOCK_ITEMS_LIST, json!({"search": "zzz"}), Some(&owner));
    assert!(none.as_array().unwrap().is_empty());
    let ledger = t.ok(m::STOCK_MOVEMENTS_LIST, json!({"item_id": id}), Some(&owner));
    let kinds: Vec<_> = ledger.as_array().unwrap().iter().map(|x| x["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"receive") && kinds.contains(&"consume") && kinds.contains(&"adjust"));
    assert!(kinds.contains(&"transfer_out") && kinds.contains(&"transfer_in"));
    let actions = t.audit_actions(&owner);
    for a in ["stock_item.create", "stock.receive", "stock.consume", "stock.adjust", "stock.transfer"] {
        assert!(actions.contains(&a.to_string()), "{a} is audited");
    }
}

#[test]
fn inventory_purchases_suppliers_and_payments_keep_the_balance_and_the_cash_right() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let cat = first_id(&t, m::STOCK_CATEGORIES_LIST, &owner, 0);
    let unit = first_id(&t, m::STOCK_UNITS_LIST, &owner, 0);
    let main = first_id(&t, m::STOCK_LOCATIONS_LIST, &owner, 0);
    let cash = method_id(&t, &owner, "cash");
    let bank = method_id(&t, &owner, "bank_transfer");
    let item = t.ok(
        m::STOCK_ITEMS_SAVE,
        json!({"name": {"fa": "کامپوزیت", "ps": "کامپوزیت", "en": "Composite"}, "category_id": cat, "unit_id": unit}),
        Some(&owner),
    );
    let item_id = s_of(&item, "id");
    let supplier =
        t.ok(m::SUPPLIERS_SAVE, json!({"kind": "supplier", "name": "Kabul Dental Supply"}), Some(&owner));
    let sid = s_of(&supplier, "id");
    let lab = t.ok(m::SUPPLIERS_SAVE, json!({"kind": "lab", "name": "Noor Lab"}), Some(&owner));
    let line = |qty: i64| json!({"item_id": item_id, "qty": qty, "unit_cost": 2500, "lot_no": "L1", "expiry_date": day(300), "location_id": main});

    // 10.00 units at 2500 = 250.00 AFN (25 000 in AFN × 100); 100.00 AFN paid in cash on the spot.
    let (_, field, rule) = rule_of(&t, m::PURCHASES_CREATE, json!({"supplier_id": sid, "lines": []}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("lines"), Some("purchase_empty")));
    let (_, field, rule) =
        rule_of(&t, m::PURCHASES_CREATE, json!({"supplier_id": lab["id"], "lines": [line(1000)]}), &owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("supplier_id"), Some("supplier_not_found")));
    let (_, field, rule) = rule_of(
        &t,
        m::PURCHASES_CREATE,
        json!({"supplier_id": sid, "lines": [line(1000)], "paid": 30_000, "method_id": cash}),
        &owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("paid"), Some("payment_exceeds")));
    let purchase = t.ok(
        m::PURCHASES_CREATE,
        json!({"supplier_id": sid, "lines": [line(1000)], "paid": 10_000, "method_id": cash}),
        Some(&owner),
    );
    assert_eq!(purchase["total"], 25_000);
    assert!(purchase["number"].as_str().unwrap().starts_with("PU-"));
    assert_eq!(t.ok(m::STOCK_ITEMS_GET, json!({"id": item_id}), Some(&owner))["qty"], 1000);
    let owed = t.ok(m::SUPPLIERS_GET, json!({"id": sid}), Some(&owner));
    assert_eq!(
        (owed["purchases_total"].as_i64(), owed["paid_total"].as_i64(), owed["balance"].as_i64()),
        (Some(25_000), Some(10_000), Some(15_000))
    );
    // The cash paid out is an expense of that day, so the cash closing sees it.
    let today = crate::clock::today_iso();
    assert_eq!(t.ok(m::CASH_DAY, json!({"date": today}), Some(&owner))["cash_expenses"], 10_000);

    // A bank payment is not cash; voiding the cash one takes its expense back.
    let bank_pay = t.ok(
        m::SUPPLIER_PAYMENTS_ADD,
        json!({"supplier_id": sid, "amount": 5_000, "method_id": bank}),
        Some(&owner),
    );
    assert_eq!(t.ok(m::CASH_DAY, json!({"date": today}), Some(&owner))["cash_expenses"], 10_000);
    assert_eq!(t.ok(m::SUPPLIERS_GET, json!({"id": sid}), Some(&owner))["balance"], 10_000);
    let payments = t.ok(m::SUPPLIER_PAYMENTS_LIST, json!({"supplier_id": sid}), Some(&owner));
    let cash_pay = payments.as_array().unwrap().iter().find(|p| p["amount"] == 10_000).unwrap().clone();
    let (_, field, rule) = rule_of(
        &t,
        m::SUPPLIER_PAYMENTS_VOID,
        json!({"id": cash_pay["id"], "version": cash_pay["version"], "reason": ""}),
        &owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("reason"), Some("required")));
    let voided_pay = t.ok(
        m::SUPPLIER_PAYMENTS_VOID,
        json!({"id": cash_pay["id"], "version": cash_pay["version"], "reason": "اشتباه"}),
        Some(&owner),
    );
    assert_eq!(t.ok(m::CASH_DAY, json!({"date": today}), Some(&owner))["cash_expenses"], 0);
    assert_eq!(t.ok(m::SUPPLIERS_GET, json!({"id": sid}), Some(&owner))["balance"], 20_000);
    let (_, _, rule) = rule_of(
        &t,
        m::SUPPLIER_PAYMENTS_VOID,
        json!({"id": cash_pay["id"], "version": voided_pay["version"], "reason": "دوباره"}),
        &owner,
    );
    assert_eq!(rule.as_deref(), Some("supplier_payment_void"));
    assert_eq!(bank_pay["amount"], 5_000);

    // Goods returned to the supplier lower what is owed.
    let batch = s_of(&t.ok(m::STOCK_BATCHES_LIST, json!({"item_id": item_id}), Some(&owner))[0], "id");
    t.ok(
        m::STOCK_RETURN_TO_SUPPLIER,
        json!({"batch_id": batch, "qty": 100, "supplier_id": sid, "credit": 2_500, "reason": "خراب"}),
        Some(&owner),
    );
    assert_eq!(t.ok(m::SUPPLIERS_GET, json!({"id": sid}), Some(&owner))["balance"], 17_500);

    // The history adds up to the balance.
    let ledger = t.ok(m::SUPPLIER_LEDGER_LIST, json!({"supplier_id": sid}), Some(&owner));
    assert_eq!(ledger.as_array().unwrap().last().unwrap()["balance"], 17_500, "{ledger}");

    // A purchase whose goods were used cannot be voided; an untouched one can, and its stock goes.
    let (_, _, rule) = rule_of(
        &t,
        m::PURCHASES_VOID,
        json!({"id": purchase["id"], "version": purchase["version"], "reason": "x"}),
        &owner,
    );
    assert_eq!(rule.as_deref(), Some("purchase_used"));
    let second = t.ok(m::PURCHASES_CREATE, json!({"supplier_id": sid, "lines": [line(200)]}), Some(&owner));
    let before = t.ok(m::STOCK_ITEMS_GET, json!({"id": item_id}), Some(&owner))["qty"].as_i64().unwrap();
    let voided = t.ok(
        m::PURCHASES_VOID,
        json!({"id": second["id"], "version": second["version"], "reason": "اشتباه ورود"}),
        Some(&owner),
    );
    assert_eq!(voided["void"], true);
    assert_eq!(
        t.ok(m::STOCK_ITEMS_GET, json!({"id": item_id}), Some(&owner))["qty"].as_i64().unwrap(),
        before - 200
    );
    assert_eq!(t.ok(m::SUPPLIERS_GET, json!({"id": sid}), Some(&owner))["balance"], 17_500);
    let (_, _, rule) = rule_of(
        &t,
        m::PURCHASES_VOID,
        json!({"id": second["id"], "version": voided["version"], "reason": "again"}),
        &owner,
    );
    assert_eq!(rule.as_deref(), Some("purchase_void"));

    let actions = t.audit_actions(&owner);
    for a in [
        "supplier.create",
        "purchase.create",
        "purchase.void",
        "supplier_payment.add",
        "supplier_payment.void",
    ] {
        assert!(actions.contains(&a.to_string()), "{a} is audited");
    }
}

#[test]
fn dental_lab_case_runs_its_cycle_warns_when_late_and_counts_towards_the_lab_balance() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    let item = treatment_done(&c, "SRG-01", 36, 350_000);
    let lab = t.ok(m::SUPPLIERS_SAVE, json!({"kind": "lab", "name": "Noor Lab"}), Some(owner));
    let goods = t.ok(m::SUPPLIERS_SAVE, json!({"kind": "supplier", "name": "Supply"}), Some(owner));
    let (_, field, rule) = rule_of(
        t,
        m::LAB_CASES_CREATE,
        json!({"patient_id": c.patient, "lab_id": goods["id"], "work_type": "Crown"}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("lab_id"), Some("supplier_not_found")));
    let case = t.ok(
        m::LAB_CASES_CREATE,
        json!({"patient_id": c.patient, "lab_id": lab["id"], "doctor_id": c.doctor, "treatment_item_id": item["id"],
               "teeth": [36], "work_type": "Crown PFM", "shade": "A2", "expected_date": day(-3), "cost": 30_000}),
        Some(owner),
    );
    assert!(case["number"].as_str().unwrap().starts_with("LC-"));
    assert_eq!((case["status"].as_str(), case["delayed"].as_bool()), (Some("planned"), Some(false)));
    let id = case["id"].clone();

    let (_, field, rule) = rule_of(
        t,
        m::LAB_CASES_SET_STATUS,
        json!({"id": id, "version": case["version"], "status": "received"}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("status"), Some("lab_status_invalid")));

    // Sent, and already past the expected date: late, and listed among the alerts.
    let sent = t.ok(
        m::LAB_CASES_SET_STATUS,
        json!({"id": id, "version": case["version"], "status": "sent"}),
        Some(owner),
    );
    assert_eq!((sent["delayed"].as_bool(), sent["days_late"].as_i64()), (Some(true), Some(3)));
    let alerts = t.ok(m::STOCK_ALERTS_LIST, json!({}), Some(owner));
    assert_eq!(alerts["lab_delayed"].as_array().unwrap().len(), 1);
    assert_eq!(
        t.ok(m::LAB_CASES_LIST, json!({"delayed_only": true}), Some(owner)).as_array().unwrap().len(),
        1
    );
    assert_eq!(
        t.ok(m::SUPPLIERS_GET, json!({"id": lab["id"]}), Some(owner))["balance"],
        0,
        "not owed until it is back"
    );

    // It comes back with the final cost: now the clinic owes the lab, and the late warning is gone.
    let back = t.ok(
        m::LAB_CASES_SET_STATUS,
        json!({"id": id, "version": sent["version"], "status": "received", "cost": 32_000}),
        Some(owner),
    );
    assert_eq!((back["delayed"].as_bool(), back["cost"].as_i64()), (Some(false), Some(32_000)));
    assert_eq!(t.ok(m::SUPPLIERS_GET, json!({"id": lab["id"]}), Some(owner))["balance"], 32_000);
    let bank = method_id(t, owner, "bank_transfer");
    t.ok(
        m::SUPPLIER_PAYMENTS_ADD,
        json!({"supplier_id": lab["id"], "amount": 12_000, "method_id": bank}),
        Some(owner),
    );
    assert_eq!(t.ok(m::SUPPLIERS_GET, json!({"id": lab["id"]}), Some(owner))["balance"], 20_000);

    // Lab cost beside what the treatment was invoiced for.
    t.ok(
        m::INVOICES_CREATE,
        json!({"patient_id": c.patient, "items": [{"item_id": item["id"]}]}),
        Some(owner),
    );
    let report = t.ok(m::LAB_CASES_REPORT, json!({"from": day(-1), "to": day(0)}), Some(owner));
    assert_eq!(
        (report["total_cost"].as_i64(), report["rows"].as_array().map(Vec::len)),
        (Some(32_000), Some(1))
    );
    assert_eq!(report["total_billed"], 350_000, "{report}");

    // Delivered to the patient: closed for editing. A cancel needs its reason.
    let done = t.ok(
        m::LAB_CASES_SET_STATUS,
        json!({"id": id, "version": back["version"], "status": "delivered"}),
        Some(owner),
    );
    let (_, _, rule) = rule_of(
        t,
        m::LAB_CASES_UPDATE,
        json!({"id": id, "version": done["version"], "lab_id": lab["id"], "work_type": "Crown", "cost": 1}),
        owner,
    );
    assert_eq!(rule.as_deref(), Some("lab_case_closed"));
    let second = t.ok(
        m::LAB_CASES_CREATE,
        json!({"patient_id": c.patient, "lab_id": lab["id"], "work_type": "Bridge", "teeth": [14, 15, 16]}),
        Some(owner),
    );
    let (_, field, rule) = rule_of(
        t,
        m::LAB_CASES_SET_STATUS,
        json!({"id": second["id"], "version": second["version"], "status": "cancelled"}),
        owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("reason"), Some("required")));
    let cancelled = t.ok(
        m::LAB_CASES_SET_STATUS,
        json!({"id": second["id"], "version": second["version"], "status": "cancelled", "reason": "بیمار نیامد"}),
        Some(owner),
    );
    assert_eq!(cancelled["status"], "cancelled");
    let actions = t.audit_actions(owner);
    for a in ["lab_case.create", "lab_case.status"] {
        assert!(actions.contains(&a.to_string()), "{a} is audited");
    }
}

#[test]
fn inventory_permissions_and_materials_of_a_service() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    t.ok(
        m::USERS_CREATE,
        json!({"username": "dr1", "display_name": "x", "password": "doctor-pass-1", "role": "doctor"}),
        Some(owner),
    );
    t.ok(
        m::USERS_CREATE,
        json!({"username": "rec1", "display_name": "y", "password": "recep-pass-12", "role": "receptionist"}),
        Some(owner),
    );
    let doctor = t.login("dr1", "doctor-pass-1");
    let reception = t.login("rec1", "recep-pass-12");
    let cat = first_id(t, m::STOCK_CATEGORIES_LIST, owner, 0);
    let unit = first_id(t, m::STOCK_UNITS_LIST, owner, 0);
    let main = first_id(t, m::STOCK_LOCATIONS_LIST, owner, 0);

    assert_eq!(t.call(m::STOCK_RECEIVE, json!({}), Some(&doctor)), Err(ErrorCode::Forbidden));
    assert_eq!(t.call(m::STOCK_ITEMS_SAVE, json!({}), Some(&reception)), Err(ErrorCode::Forbidden));
    assert_eq!(t.call(m::SUPPLIER_PAYMENTS_ADD, json!({}), Some(&doctor)), Err(ErrorCode::Forbidden));
    t.ok(m::STOCK_ITEMS_LIST, json!({}), Some(&doctor));
    t.ok(m::STOCK_ITEMS_LIST, json!({}), Some(&reception));
    assert_eq!(
        t.call(m::STOCK_CONSUME, json!({"item_id": "x", "qty": 1}), Some(&reception)),
        Err(ErrorCode::Forbidden)
    );

    let glue = t.ok(
        m::STOCK_ITEMS_SAVE,
        json!({"name": {"fa": "بوند", "ps": "بوند", "en": "Bond"}, "category_id": cat, "unit_id": unit}),
        Some(owner),
    );
    let glue_id = s_of(&glue, "id");
    t.ok(
        m::STOCK_RECEIVE,
        json!({"item_id": glue_id, "qty": 500, "unit_cost": 1000, "location_id": main}),
        Some(owner),
    );
    let item = treatment_done(&c, "SRG-01", 46, 80_000);
    let service = item["service_id"].clone();
    let (_, _, rule) =
        rule_of(t, m::STOCK_CONSUME_TREATMENT, json!({"treatment_item_id": item["id"]}), owner);
    assert_eq!(rule.as_deref(), Some("materials_none"));

    t.ok(
        m::SERVICE_MATERIALS_SAVE,
        json!({"service_id": service, "materials": [{"item_id": glue_id, "qty": 200}]}),
        Some(owner),
    );
    assert_eq!(
        t.ok(m::SERVICE_MATERIALS_LIST, json!({"service_id": service}), Some(&doctor))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // A doctor takes the materials of the treatment; the second try is refused.
    let moves = t.ok(m::STOCK_CONSUME_TREATMENT, json!({"treatment_item_id": item["id"]}), Some(&doctor));
    assert_eq!(moves[0]["qty"], -200);
    assert_eq!(moves[0]["patient_id"], json!(c.patient));
    assert_eq!(t.ok(m::STOCK_ITEMS_GET, json!({"id": glue_id}), Some(owner))["qty"], 300);
    let (_, _, rule) =
        rule_of(t, m::STOCK_CONSUME_TREATMENT, json!({"treatment_item_id": item["id"]}), owner);
    assert_eq!(rule.as_deref(), Some("materials_already_consumed"));
}

// ───────────────────────────── Phase 9: dashboard, reports, notifications, search ─────────────────────────────

const ALL_REPORTS: &[&str] = &[
    "daily_revenue",
    "monthly_revenue",
    "payments",
    "outstanding",
    "discounts",
    "refunds",
    "doctor_revenue",
    "doctor_commission",
    "service_revenue",
    "expenses",
    "profit_loss",
    "cash_closings",
    "patients",
    "treatments",
    "procedures",
    "tooth_conditions",
    "doctor_activity",
    "follow_ups",
    "recall_list",
    "referral_sources",
    "stock",
    "low_stock",
    "purchases",
    "consumption",
    "expiry",
    "supplier_balance",
];

/// A treatment done today for 350 000, invoiced with a 20 000 discount, 100 000 paid in cash.
fn invoiced_and_part_paid(c: &Clinic4) -> Value {
    let owner = c.owner.as_str();
    let item = treatment_done(c, "SRG-01", 36, 350_000);
    let inv = c.t.ok(
        m::INVOICES_CREATE,
        json!({"patient_id": c.patient, "items": [{"item_id": item["id"]}], "discount": 20_000}),
        Some(owner),
    );
    let cash = method_id(&c.t, owner, "cash");
    c.t.ok(
        m::PAYMENTS_ADD,
        json!({"patient_id": c.patient, "invoice_id": inv["id"], "amount": 100_000, "method_id": cash}),
        Some(owner),
    );
    inv
}

#[test]
fn dashboard_shows_the_clinic_to_the_owner_and_only_their_own_work_to_a_doctor() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    invoiced_and_part_paid(&c);

    let d = t.ok(m::DASHBOARD_GET, json!({}), Some(owner));
    assert_eq!(d["scope"], "clinic");
    assert_eq!(d["treatments_completed"], 1, "{d}");
    assert_eq!((d["revenue_basis"].as_str(), d["revenue_today"].as_i64()), (Some("received"), Some(100_000)));
    assert_eq!(d["outstanding"], 230_000, "350 000 - 20 000 discount - 100 000 paid: {d}");
    assert_eq!(d["license"], "unactivated", "a fresh install is in its free first period");

    // A doctor with a login sees only their own work and no cash desk.
    let user = t.ok(
        m::USERS_CREATE,
        json!({"username": "dr.ahmad", "display_name": "دکتر احمد", "password": "doctor-pass-1", "role": "doctor"}),
        Some(owner),
    );
    let doctors = t.ok(m::DOCTORS_LIST, json!({"include_inactive": false}), Some(owner));
    t.ok(
        m::DOCTORS_UPDATE,
        json!({"id": c.doctor, "version": doctors[0]["version"], "full_name": "دکتر احمد", "color": "#0e7490", "status": "active", "user_id": user["id"]}),
        Some(owner),
    );
    let doctor = t.login("dr.ahmad", "doctor-pass-1");
    let own = t.ok(m::DASHBOARD_GET, json!({}), Some(&doctor));
    assert_eq!((own["scope"].as_str(), own["doctor_name"].as_str()), (Some("doctor"), Some("دکتر احمد")));
    assert_eq!(
        (own["revenue_basis"].as_str(), own["revenue_today"].as_i64()),
        (Some("invoiced"), Some(350_000))
    );
    assert!(own["outstanding"].is_null() && own["low_stock"].is_null() && own["backup"].is_null(), "{own}");
    assert_eq!(own["treatments_completed"], 1);

    // The other doctor did nothing today.
    let user2 = t.ok(
        m::USERS_CREATE,
        json!({"username": "dr.zarghona", "display_name": "دکتر زرغونه", "password": "doctor-pass-2", "role": "doctor"}),
        Some(owner),
    );
    let doctors = t.ok(m::DOCTORS_LIST, json!({"include_inactive": false}), Some(owner));
    let second = doctors.as_array().unwrap().iter().find(|x| x["id"] == json!(c.doctor2)).unwrap();
    t.ok(
        m::DOCTORS_UPDATE,
        json!({"id": c.doctor2, "version": second["version"], "full_name": "دکتر زرغونه", "color": "#f59e0b", "status": "active", "user_id": user2["id"]}),
        Some(owner),
    );
    let other = t.login("dr.zarghona", "doctor-pass-2");
    let none = t.ok(m::DASHBOARD_GET, json!({}), Some(&other));
    assert_eq!((none["treatments_completed"].as_i64(), none["revenue_today"].as_i64()), (Some(0), Some(0)));
}

#[test]
fn every_report_runs_and_the_money_ones_add_up() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    invoiced_and_part_paid(&c);
    let (from, to) = (day(-30), day(0));

    for kind in ALL_REPORTS {
        let r = t.ok(m::REPORTS_RUN, json!({"kind": kind, "from": from, "to": to}), Some(owner));
        let cols = r["columns"].as_array().unwrap().len();
        assert!(cols > 0, "{kind} has columns");
        for row in r["rows"].as_array().unwrap() {
            assert_eq!(row.as_array().unwrap().len(), cols, "{kind}: every row has a cell per column");
        }
        let totals = r["totals"].as_array().unwrap();
        assert!(totals.is_empty() || totals.len() == cols, "{kind}: totals line up with the columns");
    }

    // Revenue by day: invoiced 330 000 (350 000 less the discount), received 100 000.
    let daily = t.ok(m::REPORTS_RUN, json!({"kind": "daily_revenue", "from": from, "to": to}), Some(owner));
    let row = &daily["rows"][0];
    assert_eq!(row[0]["value"], json!(day(0)));
    assert_eq!(
        (row[2]["value"].as_i64(), row[3]["value"].as_i64(), row[5]["value"].as_i64()),
        (Some(330_000), Some(100_000), Some(100_000))
    );
    assert_eq!(daily["totals"][3]["value"], 100_000);
    // Monthly and profit and loss see the same money.
    let month = t.ok(
        m::REPORTS_RUN,
        json!({"kind": "monthly_revenue", "from": from, "to": to, "calendar": "shamsi"}),
        Some(owner),
    );
    assert_eq!(month["totals"][3]["value"], 100_000);
    let pl = t.ok(m::REPORTS_RUN, json!({"kind": "profit_loss", "from": from, "to": to}), Some(owner));
    assert_eq!(pl["totals"][4]["value"], 100_000, "no expenses yet: profit = received");
    // The discount and the debtor.
    let discounts = t.ok(m::REPORTS_RUN, json!({"kind": "discounts", "from": from, "to": to}), Some(owner));
    assert_eq!(discounts["rows"][0][4]["value"], 20_000);
    let owed = t.ok(m::REPORTS_RUN, json!({"kind": "outstanding", "from": from, "to": to}), Some(owner));
    assert_eq!(owed["totals"][6]["value"], 230_000);
    // By doctor, and narrowed to the other doctor.
    let by_doctor =
        t.ok(m::REPORTS_RUN, json!({"kind": "doctor_revenue", "from": from, "to": to}), Some(owner));
    assert_eq!(by_doctor["rows"][0][3]["value"], 350_000);
    let other = t.ok(
        m::REPORTS_RUN,
        json!({"kind": "doctor_revenue", "from": from, "to": to, "doctor_id": c.doctor2}),
        Some(owner),
    );
    assert!(other["rows"].as_array().unwrap().is_empty());
    let done = t.ok(m::REPORTS_RUN, json!({"kind": "treatments", "from": from, "to": to}), Some(owner));
    assert_eq!(done["rows"].as_array().unwrap().len(), 1);
    assert_eq!(done["rows"][0][0]["value"], json!(day(0)), "shown on the clinic's day");

    // A wrong period is refused with its rule.
    let (_, field, rule) =
        rule_of(t, m::REPORTS_RUN, json!({"kind": "payments", "from": to, "to": from}), owner);
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("to"), Some("period_range")));

    // Excel: a real file in the exports folder, audited.
    let file = t.ok(
        m::REPORTS_EXPORT,
        json!({"report": {"kind": "daily_revenue", "from": from, "to": to}, "language": "fa", "title": "Daily",
               "headers": ["a", "b", "c", "d", "e", "f"], "labels": {}}),
        Some(owner),
    );
    assert!(file["file_name"].as_str().unwrap().ends_with(".xlsx"));
    assert!(std::path::Path::new(file["file_path"].as_str().unwrap()).exists());
    assert!(t.audit_actions(owner).contains(&"report.export".to_string()));
}

#[test]
fn reports_need_their_permissions() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    for (user, role) in [("recep", "receptionist"), ("acct", "accountant")] {
        t.ok(
            m::USERS_CREATE,
            json!({"username": user, "display_name": user, "password": "some-pass-12", "role": role}),
            Some(owner),
        );
    }
    let (from, to) = (day(-7), day(0));
    let recep = t.login("recep", "some-pass-12");
    assert_eq!(
        t.call(m::REPORTS_RUN, json!({"kind": "payments", "from": from, "to": to}), Some(&recep)),
        Err(ErrorCode::Forbidden)
    );
    let acct = t.login("acct", "some-pass-12");
    t.ok(m::REPORTS_RUN, json!({"kind": "payments", "from": from, "to": to}), Some(&acct));
    t.ok(m::REPORTS_RUN, json!({"kind": "stock", "from": from, "to": to}), Some(&acct));
    // The full audit log stays with those who may see it.
    assert_eq!(
        t.call(m::AUDIT_LIST, json!({"limit": 10, "offset": 0}), Some(&acct)),
        Err(ErrorCode::Forbidden)
    );
}

#[test]
fn notifications_list_what_needs_attention_and_remember_what_was_read() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    t.ok(
        m::RECALLS_CREATE,
        json!({"patient_id": c.patient, "kind": "follow_up", "due_date": day(-2), "note": "check the extraction"}),
        Some(owner),
    );
    invoiced_and_part_paid(&c);
    let list = t.ok(m::NOTIFICATIONS_LIST, json!({}), Some(owner));
    let kinds: Vec<&str> =
        list["items"].as_array().unwrap().iter().map(|i| i["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"follow_up_due"), "{kinds:?}");
    let unread = list["unread"].as_i64().unwrap();
    assert_eq!(unread, list["items"].as_array().unwrap().len() as i64);

    // One read; then everything.
    let key = list["items"][0]["key"].as_str().unwrap().to_string();
    let after = t.ok(m::NOTIFICATIONS_MARK_READ, json!({"keys": [key]}), Some(owner));
    assert_eq!(after["unread"], unread - 1);
    let all = t.ok(m::NOTIFICATIONS_MARK_READ, json!({}), Some(owner));
    assert_eq!(all["unread"], 0);
    assert!(all["items"].as_array().unwrap().iter().all(|i| i["read"] == true));
    // Reading is per user, and reception never sees backups.
    t.ok(
        m::USERS_CREATE,
        json!({"username": "recep", "display_name": "پذیرش", "password": "recep-pass-1", "role": "receptionist"}),
        Some(owner),
    );
    let recep = t.login("recep", "recep-pass-1");
    let theirs = t.ok(m::NOTIFICATIONS_LIST, json!({}), Some(&recep));
    assert!(theirs["unread"].as_i64().unwrap() > 0);
    let kinds: Vec<&str> =
        theirs["items"].as_array().unwrap().iter().map(|i| i["kind"].as_str().unwrap()).collect();
    assert!(!kinds.contains(&"backup_overdue"), "reception may not see backups: {kinds:?}");
}

#[test]
fn global_search_finds_patients_documents_and_respects_access() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    let inv = invoiced_and_part_paid(&c);
    let number = inv["number"].as_str().unwrap().to_string();

    let by_name = t.ok(m::SEARCH_GLOBAL, json!({"query": "بیمار یکم"}), Some(owner));
    let kinds: Vec<&str> =
        by_name["hits"].as_array().unwrap().iter().map(|h| h["kind"].as_str().unwrap()).collect();
    assert!(
        kinds.contains(&"patient") && kinds.contains(&"invoice") && kinds.contains(&"receipt"),
        "{kinds:?}"
    );
    let by_number = t.ok(m::SEARCH_GLOBAL, json!({"query": number}), Some(owner));
    assert!(by_number["hits"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["kind"] == "invoice" && h["title"] == json!(number)));
    // One letter is not a search.
    assert!(t.ok(m::SEARCH_GLOBAL, json!({"query": "ب"}), Some(owner))["hits"]
        .as_array()
        .unwrap()
        .is_empty());

    // An assistant sees patients but never money.
    t.ok(
        m::USERS_CREATE,
        json!({"username": "assist", "display_name": "دستیار", "password": "assist-pass-1", "role": "assistant"}),
        Some(owner),
    );
    let assist = t.login("assist", "assist-pass-1");
    let hits = t.ok(m::SEARCH_GLOBAL, json!({"query": "بیمار یکم"}), Some(&assist));
    let kinds: Vec<&str> =
        hits["hits"].as_array().unwrap().iter().map(|h| h["kind"].as_str().unwrap()).collect();
    assert!(
        kinds.contains(&"patient") && !kinds.contains(&"invoice") && !kinds.contains(&"receipt"),
        "{kinds:?}"
    );
}

#[test]
fn the_audit_viewer_filters_and_marks_the_sensitive_actions() {
    let c = clinic4();
    let (t, owner) = (&c.t, c.owner.as_str());
    let mut update = t.ok(m::PATIENTS_GET, json!({"patient_id": c.patient}), Some(owner));
    update["full_name"] = json!("بیمار یکم (ویرایش)");
    update["status"] = json!("active");
    t.ok(m::PATIENTS_UPDATE, update, Some(owner));
    invoiced_and_part_paid(&c);

    let page = |extra: Value| {
        let mut p = json!({"limit": 200, "offset": 0});
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        t.ok(m::AUDIT_LIST, p, Some(owner))
    };
    let invoice_only = page(json!({"action": "invoice"}));
    assert!(invoice_only
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["action"].as_str().unwrap().contains("invoice")));
    assert!(!invoice_only.as_array().unwrap().is_empty());
    let sensitive = page(json!({"sensitive_only": true}));
    let acts: Vec<&str> =
        sensitive.as_array().unwrap().iter().map(|e| e["action"].as_str().unwrap()).collect();
    assert!(acts.contains(&"patient.update"), "{acts:?}");
    assert!(!acts.contains(&"invoice.create"), "creating an invoice is routine: {acts:?}");
    assert!(sensitive.as_array().unwrap().iter().all(|e| e["sensitive"] == true));
    // The same flag in the unfiltered list, and the user and date filters.
    let everything = page(json!({}));
    assert!(everything
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["action"] == "patient.update" && e["sensitive"] == true));
    assert!(!page(json!({"username": "owner", "from": day(0), "to": day(0)})).as_array().unwrap().is_empty());
    assert!(page(json!({"username": "nobody"})).as_array().unwrap().is_empty());
    assert!(page(json!({"from": day(-10), "to": day(-9)})).as_array().unwrap().is_empty());
    // A percent sign in the text is just a percent sign, not a wildcard.
    assert!(page(json!({"action": "%"})).as_array().unwrap().is_empty());
}

#[test]
fn sensitive_actions_are_recognised_by_name() {
    use crate::audit::is_sensitive;
    for a in [
        "patient.delete",
        "invoice.void",
        "purchase.void",
        "payment.refund",
        "role.update",
        "user.reset_password",
        "expense.delete",
        "patient.update",
    ] {
        assert!(is_sensitive(a), "{a}");
    }
    for a in ["invoice.create", "payment.print", "auth.login", "recall.create", "stock.receive"] {
        assert!(!is_sensitive(a), "{a}");
    }
}
