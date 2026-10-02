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

    let preview = t.ok(m::PATIENTS_IMPORT, json!({"csv_base64": csv_base64, "commit": false}), Some(&owner));
    assert_eq!(preview["total"], 3);
    assert_eq!(preview["imported"], 0);
    assert_eq!(preview["skipped"], 1);
    assert_eq!(t.ok(m::PATIENTS_LIST, json!({}), Some(&owner))["total"], 0);

    let result = t.ok(m::PATIENTS_IMPORT, json!({"csv_base64": csv_base64, "commit": true}), Some(&owner));
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
