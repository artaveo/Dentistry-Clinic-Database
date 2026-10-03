//! End-to-end tests of the Core through its public RPC entry point, plus
//! a few checks that need internal access (append-only audit, migrations).

use std::time::Duration;

use artaveo_shared::{methods as m, ErrorCode, RpcRequest, RpcResponse};
use serde_json::{json, Value};

use crate::keys::{default_protector, RecoveryKey};
use crate::{db, Config, Core, Environment};

struct T {
    _dir: tempfile::TempDir,
    core: Core,
}

impl T {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::open(Config::new(Environment::Test, dir.path().to_path_buf())).unwrap();
        T { _dir: dir, core }
    }

    fn reopen(self) -> Self {
        let dir = self._dir;
        drop(self.core);
        let core = Core::open(Config::new(Environment::Test, dir.path().to_path_buf())).unwrap();
        T { _dir: dir, core }
    }

    fn call(&self, method: &str, params: Value, token: Option<&str>) -> Result<Value, ErrorCode> {
        match self.core.handle(RpcRequest { method: method.into(), params, token: token.map(Into::into) }) {
            RpcResponse::Ok { result } => Ok(result),
            RpcResponse::Error { error } => Err(error.code),
        }
    }

    fn ok(&self, method: &str, params: Value, token: Option<&str>) -> Value {
        self.call(method, params, token).unwrap_or_else(|e| panic!("{method} failed: {e:?}"))
    }

    fn setup(&self) -> String {
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

    fn login(&self, user: &str, pass: &str) -> String {
        self.ok(m::AUTH_LOGIN, json!({"username": user, "password": pass}), None)["token"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn audit_actions(&self, token: &str) -> Vec<String> {
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
fn backup_retention_keeps_newest() {
    let t = T::new();
    t.setup();
    let tok = t.login("owner", "owner-pass-1");
    let mut s = t.ok(m::SETTINGS_GET, json!({}), Some(&tok));
    s["backup_keep_daily"] = json!(2);
    t.ok(m::SETTINGS_UPDATE, s, Some(&tok));
    let mut names = vec![];
    for _ in 0..3 {
        names.push(t.ok(m::BACKUP_CREATE, json!({}), Some(&tok))["file_name"].as_str().unwrap().to_string());
        std::thread::sleep(Duration::from_millis(1100)); // file names have 1 s resolution
    }
    let listed = t.ok(m::BACKUP_LIST, json!({}), Some(&tok));
    assert_eq!(listed.as_array().unwrap().len(), 2);
    assert!(!t.core.config().backup_dir().join(&names[0]).exists());
    assert!(t.core.config().backup_dir().join(&names[2]).exists());
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

#[test]
fn medical_history_alert_and_optimistic_locking() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let patient = t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad Khan"}), Some(&owner));

    let empty = t.ok(m::MEDICAL_HISTORY_GET, json!({"patient_id": patient["id"]}), Some(&owner));
    assert_eq!(empty["version"], 0);

    let created = t.ok(
        m::MEDICAL_HISTORY_UPDATE,
        json!({"patient_id": patient["id"], "version": 0, "allergies": "Penicillin", "chronic_conditions": "Diabetes"}),
        Some(&owner),
    );
    assert_eq!(created["version"], 1);
    assert_eq!(created["allergies"], "Penicillin");

    assert_eq!(
        t.call(
            m::MEDICAL_HISTORY_UPDATE,
            json!({"patient_id": patient["id"], "version": 0, "allergies": "x"}),
            Some(&owner)
        ),
        Err(ErrorCode::Conflict)
    );

    let updated = t.ok(
        m::MEDICAL_HISTORY_UPDATE,
        json!({"patient_id": patient["id"], "version": 1, "allergies": "Penicillin, Latex"}),
        Some(&owner),
    );
    assert_eq!(updated["version"], 2);

    let actions = t.audit_actions(&owner);
    assert_eq!(actions.iter().filter(|a| a.as_str() == "patient.medical_history_update").count(), 2);
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

    let exported = t.ok(m::PATIENTS_EXPORT, json!({}), Some(&owner));
    let bytes = data_encoding::BASE64.decode(exported["csv_base64"].as_str().unwrap().as_bytes()).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("Ahmad Khan"));
    assert!(text.contains("Zarghuna"));
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
    let edited = c.t.ok(
        m::APPOINTMENTS_UPDATE,
        json!({"id": a["id"], "version": a["version"], "doctor_id": c.doctor, "date": day, "start_time": "11:00", "end_time": "11:30", "notes": "bring X-rays"}),
        Some(&c.owner),
    );
    assert_eq!(edited["notes"], "bring X-rays");
    // Drag & drop is the same call with a new time; the old slot becomes free.
    let moved = c.t.ok(
        m::APPOINTMENTS_UPDATE,
        json!({"id": a["id"], "version": edited["version"], "doctor_id": c.doctor, "date": day, "start_time": "13:00", "end_time": "13:30"}),
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
    assert!(c.t.call(m::APPOINTMENTS_UPDATE, json!({"id": a["id"], "version": a["version"], "doctor_id": c.doctor, "date": c.today, "start_time": "09:00", "end_time": "09:30", "notes": "late"}), Some(&c.owner)).is_ok());

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

    // Built-in roles are fixed; a role in use cannot be deleted.
    let (_, _, rule) = rule_of(
        &t,
        m::ROLES_UPDATE,
        json!({"code": "receptionist", "version": 1, "label": "x", "permissions": ["patients.view"]}),
        &owner,
    );
    assert_eq!(rule.as_deref(), Some("role_system"));
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
