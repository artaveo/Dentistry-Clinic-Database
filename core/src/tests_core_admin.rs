//! Tests for the fixes of the admin/patients/backup test mission (R-*, PA-*, BR-* ids in the comments;
//! see docs/test-mission/FIXES-core-admin.md).

use artaveo_shared::{methods as m, ErrorCode, RpcRequest, RpcResponse};
use serde_json::{json, Value};

use crate::tests::T;

fn id_of(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn first_id(t: &T, method: &str, token: &str) -> String {
    id_of(&t.ok(method, json!({"include_inactive": false}), Some(token))[0])
}

/// Full error of a call that must fail: (code, field, rule).
fn failure(t: &T, method: &str, params: Value, token: &str) -> (ErrorCode, Option<String>, Option<String>) {
    match t.core.handle(RpcRequest { method: method.into(), params, token: Some(token.into()) }) {
        RpcResponse::Ok { .. } => panic!("{method} unexpectedly succeeded"),
        RpcResponse::Error { error } => (
            error.code,
            error.field.clone(),
            error.rule.map(|r| serde_json::to_value(r).unwrap().as_str().unwrap().to_string()),
        ),
    }
}

fn day(n: i64) -> String {
    let d = crate::scheduling::parse_date("d", &crate::clock::today_iso()).unwrap() + time::Duration::days(n);
    crate::scheduling::format_date(d)
}

fn new_user(t: &T, owner: &str, username: &str, role: &str, pass: &str) -> Value {
    t.ok(
        m::USERS_CREATE,
        json!({"username": username, "display_name": username, "password": pass, "role": role}),
        Some(owner),
    )
}

/// Owner + one user of every other system role, logged in. Returns (t, owner token, tokens by role).
fn staff() -> (T, String, Vec<(&'static str, String)>) {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let mut tokens = Vec::new();
    for role in ["administrator", "receptionist", "doctor", "accountant", "assistant"] {
        new_user(&t, &owner, &format!("u.{role}"), role, "staff-pass-12");
        tokens.push((role, t.login(&format!("u.{role}"), "staff-pass-12")));
    }
    (t, owner, tokens)
}

fn token<'a>(tokens: &'a [(&'static str, String)], role: &str) -> &'a str {
    &tokens.iter().find(|(r, _)| *r == role).unwrap().1
}

// ───────────────────────────── R-11 ─────────────────────────────

#[test]
fn administrator_cannot_touch_the_owner_account() {
    let (t, owner, tokens) = staff();
    let admin = token(&tokens, "administrator");
    let me = t.ok(m::SESSION_STATE, json!({}), Some(&owner));
    assert_eq!(me["locked"], false);
    let users = t.ok(m::USERS_LIST, json!({}), Some(admin));
    let owner_row = users.as_array().unwrap().iter().find(|u| u["role"] == "owner").unwrap().clone();

    // Reset the owner's password: refused, and the old password still works.
    assert_eq!(
        t.call(
            m::USERS_RESET_PASSWORD,
            json!({"id": owner_row["id"], "new_password": "takeover-pass-1"}),
            Some(admin)
        ),
        Err(ErrorCode::Forbidden)
    );
    t.login("owner", "owner-pass-1");
    // Edit the owner's name: refused.
    assert_eq!(
        t.call(
            m::USERS_UPDATE,
            json!({"id": owner_row["id"], "version": owner_row["version"], "display_name": "hacked",
                   "role": "owner", "is_active": true}),
            Some(admin)
        ),
        Err(ErrorCode::Forbidden)
    );
    // The owner can still do both to themselves.
    t.ok(
        m::USERS_UPDATE,
        json!({"id": owner_row["id"], "version": owner_row["version"], "display_name": "New Name",
               "role": "owner", "is_active": true}),
        Some(&owner),
    );
    // An administrator can still manage other users.
    let rec = users.as_array().unwrap().iter().find(|u| u["username"] == "u.receptionist").unwrap().clone();
    t.ok(m::USERS_RESET_PASSWORD, json!({"id": rec["id"], "new_password": "another-pass-1"}), Some(admin));
}

// ───────────────────────────── R-01 / R-21 ─────────────────────────────

#[test]
fn patient_export_and_purchase_data_are_limited_to_the_right_roles() {
    let (t, owner, tokens) = staff();
    for role in ["receptionist", "doctor", "accountant", "assistant"] {
        assert_eq!(
            t.call(m::PATIENTS_EXPORT, json!({}), Some(token(&tokens, role))),
            Err(ErrorCode::Forbidden),
            "{role}"
        );
    }
    t.ok(m::PATIENTS_EXPORT, json!({}), Some(token(&tokens, "administrator")));
    t.ok(m::PATIENTS_EXPORT, json!({}), Some(&owner));

    let supplier = t.ok(m::SUPPLIERS_SAVE, json!({"kind": "supplier", "name": "Kabul Supply"}), Some(&owner));
    let sid = id_of(&supplier);
    for role in ["receptionist", "doctor", "assistant"] {
        let tok = token(&tokens, role);
        assert_eq!(t.call(m::PURCHASES_LIST, json!({}), Some(tok)), Err(ErrorCode::Forbidden), "{role}");
        assert_eq!(
            t.call(m::FIN_VOUCHERS_LIST, json!({}), Some(tok)),
            Err(ErrorCode::Forbidden),
            "{role}"
        );
        assert_eq!(
            t.call(m::SUPPLIER_LEDGER_LIST, json!({"supplier_id": sid}), Some(tok)),
            Err(ErrorCode::Forbidden),
            "{role}"
        );
        // The name is still there for the pickers, the money is not.
        let seen = t.ok(m::SUPPLIERS_GET, json!({"id": sid}), Some(tok));
        assert_eq!(seen["name"], "Kabul Supply");
        assert_eq!(seen["balance"], 0);
    }
    t.ok(m::PURCHASES_LIST, json!({}), Some(token(&tokens, "accountant")));
}

// ───────────────────────────── R-12 ─────────────────────────────

#[test]
fn changing_the_password_needs_a_new_one_and_ends_other_sessions() {
    let t = T::new();
    t.setup();
    let a = t.login("owner", "owner-pass-1");
    let b = t.login("owner", "owner-pass-1");
    let (code, field, _) = failure(
        &t,
        m::AUTH_CHANGE_PASSWORD,
        json!({"current_password": "owner-pass-1", "new_password": "owner-pass-1"}),
        &a,
    );
    assert_eq!((code, field.as_deref()), (ErrorCode::Validation, Some("new_password")));
    t.ok(
        m::AUTH_CHANGE_PASSWORD,
        json!({"current_password": "owner-pass-1", "new_password": "owner-pass-2"}),
        Some(&a),
    );
    t.ok(m::SESSION_STATE, json!({}), Some(&a));
    assert_eq!(t.call(m::SESSION_STATE, json!({}), Some(&b)), Err(ErrorCode::SessionExpired));
    t.login("owner", "owner-pass-2");
}

// ───────────────────────────── R-02 / R-04 / R-13 ─────────────────────────────

#[test]
fn stock_item_codes_are_unique_and_nearest_expiry_ignores_expired_lots() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let cat = first_id(&t, m::STOCK_CATEGORIES_LIST, &owner);
    let unit = first_id(&t, m::STOCK_UNITS_LIST, &owner);
    let place = first_id(&t, m::STOCK_LOCATIONS_LIST, &owner);
    let save = |code: &str| {
        t.call(
            m::STOCK_ITEMS_SAVE,
            json!({"code": code, "name": {"fa": "الف", "ps": "الف", "en": "A"}, "category_id": cat, "unit_id": unit}),
            Some(&owner),
        )
    };
    let first = save("NEEDLE-1").unwrap();
    assert_eq!(save("needle-1"), Err(ErrorCode::Validation), "same code, any case");
    save("NEEDLE-2").unwrap();
    // Saving the item itself again keeps its own code.
    t.ok(
        m::STOCK_ITEMS_SAVE,
        json!({"id": first["id"], "version": first["version"], "code": "NEEDLE-1",
               "name": {"fa": "الف", "ps": "الف", "en": "A"}, "category_id": cat, "unit_id": unit}),
        Some(&owner),
    );

    let id = id_of(&first);
    let receive = |expiry: String| {
        t.ok(
            m::STOCK_RECEIVE,
            json!({"item_id": id, "qty": 100, "unit_cost": 10, "expiry_date": expiry, "location_id": place}),
            Some(&owner),
        )
    };
    receive(day(-30));
    let item = receive(day(40));
    assert_eq!(
        item["nearest_expiry"],
        json!(day(40)),
        "the expired lot cannot be used, so it is not 'nearest'"
    );
}

#[test]
fn clinic_phone_and_address_are_validated() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let clinic = t.ok(m::CLINIC_GET, json!({}), Some(&owner));
    let mut bad = clinic.clone();
    bad["phone"] = json!("abc");
    assert_eq!(t.call(m::CLINIC_UPDATE, bad, Some(&owner)), Err(ErrorCode::Validation));
    let mut long = clinic.clone();
    long["address"] = json!("x".repeat(301));
    assert_eq!(t.call(m::CLINIC_UPDATE, long, Some(&owner)), Err(ErrorCode::Validation));
    let mut good = clinic;
    good["phone"] = json!("0700 123 456");
    t.ok(m::CLINIC_UPDATE, good, Some(&owner));
}

// ───────────────────────────── patients ─────────────────────────────

#[test]
fn patient_input_is_validated_and_search_finds_phone_and_joined_names() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let create = |body: Value| t.call(m::PATIENTS_CREATE, body, Some(&owner));
    // PA-03 birth dates, PA-07 registration dates, PA-15 names.
    assert_eq!(
        create(json!({"full_name": "Ali", "date_of_birth": "2031-01-01"})),
        Err(ErrorCode::Validation)
    );
    assert_eq!(
        create(json!({"full_name": "Ali", "date_of_birth": "1800-01-01"})),
        Err(ErrorCode::Validation)
    );
    assert_eq!(
        create(json!({"full_name": "Ali", "registration_date": day(400)})),
        Err(ErrorCode::Validation)
    );
    assert_eq!(create(json!({"full_name": "12345"})), Err(ErrorCode::Validation));
    assert_eq!(create(json!({"full_name": "<script>x</script>"})), Err(ErrorCode::Validation));
    // PA-16: an unknown list id is a validation error, not a raw database error.
    assert_eq!(create(json!({"full_name": "Ali Reza", "gender_id": "nope"})), Err(ErrorCode::Validation));

    let p =
        create(json!({"full_name": "علی‌رضا احمدی", "phone": "0700 123 456", "date_of_birth": "1990-05-20"}))
            .unwrap();
    create(json!({"full_name": "کریم یوسفی", "phone": "+93 788 555 666"})).unwrap();
    let find = |q: &str| {
        t.ok(m::PATIENTS_LIST, json!({"query": q, "limit": 50, "offset": 0}), Some(&owner))["items"]
            .as_array()
            .unwrap()
            .len()
    };
    // PA-01: the phone as shown, with a dash, without the 0, and as a prefix.
    assert_eq!(find("0700 123 456"), 1);
    assert_eq!(find("0700-123-456"), 1);
    assert_eq!(find("700123456"), 1);
    assert_eq!(find("0700123"), 1);
    assert_eq!(find("788555666"), 1, "stored with +93");
    // PA-02: half-space in the stored name, joined in the query (and the reverse).
    assert_eq!(find("علیرضا"), 1);
    assert_eq!(find("علی رضا"), 1);
    assert_eq!(find("علی‌رضا"), 1);
    assert_eq!(find("کریم"), 1);
    assert_eq!(find("nobody"), 0);
    let _ = p;
}

#[test]
fn search_index_is_rebuilt_once_for_an_old_database() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    t.ok(m::PATIENTS_CREATE, json!({"full_name": "علی‌رضا احمدی", "phone": "0700123456"}), Some(&owner));
    // Pretend the index was built by the old version: no joined spelling, no version mark.
    t.core
        .with_db(|o| {
            o.conn.execute("DELETE FROM patient_fts", [])?;
            o.conn.execute(
                "INSERT INTO patient_fts(patient_id, search_text)
             SELECT id, 'old ' || patient_number FROM patient",
                [],
            )?;
            o.conn.execute("DELETE FROM setting WHERE key = 'patient_fts_version'", [])?;
            Ok(())
        })
        .unwrap();
    let t = t.reopen();
    let owner = t.login("owner", "owner-pass-1");
    let found = t.ok(m::PATIENTS_LIST, json!({"query": "علیرضا", "limit": 10, "offset": 0}), Some(&owner));
    assert_eq!(found["items"].as_array().unwrap().len(), 1);
}

#[test]
fn a_patient_with_records_cannot_be_deleted_and_a_merged_record_cannot_be_merged_again() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let mk = |name: &str, phone: &str| {
        t.ok(
            m::PATIENTS_CREATE,
            json!({"full_name": name, "phone": phone, "allow_duplicate": true}),
            Some(&owner),
        )
    };
    let (a, b, c, d) = (
        mk("Ahmad One", "0700000001"),
        mk("Ahmad Two", "0700000002"),
        mk("Ahmad Three", "0700000003"),
        mk("Ahmad Four", "0700000004"),
    );
    // PA-09: an uploaded image is part of the record.
    t.ok(
        m::ATTACHMENTS_UPLOAD,
        json!({"patient_id": a["id"], "kind": "xray", "file_name": "x.png",
               "data_base64": data_encoding::BASE64.encode(b"\x89PNG fake")}),
        Some(&owner),
    );
    assert_eq!(
        t.call(m::PATIENTS_DELETE, json!({"id": a["id"], "version": a["version"]}), Some(&owner)),
        Err(ErrorCode::Validation)
    );
    // A patient without any record is still deleted.
    t.ok(m::PATIENTS_DELETE, json!({"id": d["id"], "version": d["version"]}), Some(&owner));

    // PA-22: a stale version of the merged-away record is refused.
    assert_eq!(
        t.call(
            m::PATIENTS_MERGE,
            json!({"keep_id": a["id"], "merge_id": b["id"], "merge_id_version": 99}),
            Some(&owner)
        ),
        Err(ErrorCode::Conflict)
    );
    t.ok(
        m::PATIENTS_MERGE,
        json!({"keep_id": a["id"], "merge_id": b["id"], "merge_id_version": b["version"]}),
        Some(&owner),
    );
    let merged = t.ok(m::PATIENTS_GET, json!({"patient_id": b["id"]}), Some(&owner));
    // PA-23: not again, and not into a merged-away record.
    let again = json!({"keep_id": c["id"], "merge_id": b["id"], "merge_id_version": merged["version"]});
    assert_eq!(failure(&t, m::PATIENTS_MERGE, again, &owner).2.as_deref(), Some("patient_merged"));
    let into = json!({"keep_id": b["id"], "merge_id": c["id"], "merge_id_version": c["version"]});
    assert_eq!(failure(&t, m::PATIENTS_MERGE, into, &owner).2.as_deref(), Some("patient_merged"));
}

// ───────────────────────────── appointments and recalls ─────────────────────────────

#[test]
fn a_future_appointment_cannot_be_a_no_show_and_a_recall_is_not_added_twice() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let doctor = t.ok(m::DOCTORS_CREATE, json!({"full_name": "دکتر احمد", "color": "#0E7490"}), Some(&owner));
    let patient =
        t.ok(m::PATIENTS_CREATE, json!({"full_name": "بیمار یکم", "phone": "0700111111"}), Some(&owner));
    let appt = t.ok(
        m::APPOINTMENTS_CREATE,
        json!({"patient_id": patient["id"], "doctor_id": doctor["id"], "date": day(5),
               "start_time": "09:00", "end_time": "09:30"}),
        Some(&owner),
    );
    assert_eq!(
        t.call(
            m::APPOINTMENTS_SET_STATUS,
            json!({"id": appt["id"], "version": appt["version"], "status": "no_show"}),
            Some(&owner)
        ),
        Err(ErrorCode::Validation)
    );
    // Cancelling a future one is still fine.
    t.ok(
        m::APPOINTMENTS_SET_STATUS,
        json!({"id": appt["id"], "version": appt["version"], "status": "cancelled"}),
        Some(&owner),
    );

    let recall = json!({"patient_id": patient["id"], "kind": "checkup", "due_date": day(30)});
    t.ok(m::RECALLS_CREATE, recall.clone(), Some(&owner));
    assert_eq!(t.call(m::RECALLS_CREATE, recall, Some(&owner)), Err(ErrorCode::Validation));
    t.ok(
        m::RECALLS_CREATE,
        json!({"patient_id": patient["id"], "kind": "checkup", "due_date": day(31)}),
        Some(&owner),
    );
}

// ───────────────────────────── backup ─────────────────────────────

#[test]
fn local_backup_carries_the_images_and_a_damaged_file_is_called_damaged() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let patient = t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad Khan"}), Some(&owner));
    t.ok(
        m::ATTACHMENTS_UPLOAD,
        json!({"patient_id": patient["id"], "kind": "xray", "file_name": "molar.png",
               "data_base64": data_encoding::BASE64.encode(b"\x89PNG fake x-ray bytes")}),
        Some(&owner),
    );
    let b = t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    let dir = t.core.config().backup_dir();
    // BR-1: the encrypted image file is beside the database copy.
    let copied = std::fs::read_dir(dir.join("attachments"))
        .unwrap()
        .flat_map(|sub| std::fs::read_dir(sub.unwrap().path()).unwrap())
        .filter(|f| f.as_ref().unwrap().file_name().to_string_lossy().ends_with(".bin"))
        .count();
    assert_eq!(copied, 1, "the image file (not an image, so no thumbnail) is beside the database copy");

    // BR-4: cut-short and non-database files are "damaged", not "wrong_key".
    let name = b["file_name"].as_str().unwrap().to_string();
    let bytes = std::fs::read(dir.join(&name)).unwrap();
    std::fs::write(dir.join("artaveo-20200101-100000-manual.db"), &bytes[..bytes.len() - 100]).unwrap();
    std::fs::write(dir.join("artaveo-20200102-100000-manual.db"), b"hello, I am not a database").unwrap();
    for file in ["artaveo-20200101-100000-manual.db", "artaveo-20200102-100000-manual.db"] {
        let p = t.ok(m::BACKUP_RESTORE_PREVIEW, json!({"source": "local", "file_name": file}), Some(&owner));
        assert_eq!((p["valid"].as_bool(), p["problem"].as_str()), (Some(false), Some("damaged")), "{file}");
    }
    let good = t.ok(m::BACKUP_RESTORE_PREVIEW, json!({"source": "local", "file_name": name}), Some(&owner));
    assert_eq!(good["valid"], true);

    // BR-2: a register row whose file is gone is marked removed.
    std::fs::remove_file(dir.join(&name)).unwrap();
    t.core.with_db(|o| crate::backup::reconcile(&o.conn, &dir)).unwrap();
    assert!(t
        .ok(m::BACKUP_LIST, json!({}), Some(&owner))
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x["file_name"] != name.as_str()));
}
