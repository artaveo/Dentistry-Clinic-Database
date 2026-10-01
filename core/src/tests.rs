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
