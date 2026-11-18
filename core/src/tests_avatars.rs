//! Pictures for patients and doctors (owner feedback OF-049, OF-050): migration 0019, the doctor's
//! sex, photo upload/limits/permissions, merge and the backup copy of the photo file.

use artaveo_shared::{methods as m, ErrorCode};
use serde_json::{json, Value};

use crate::tests::T;

fn png(w: u32, h: u32) -> String {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([14, 116, 144, 255]));
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    data_encoding::BASE64.encode(&out)
}

fn reception(t: &T, owner: &str) -> String {
    t.ok(
        m::USERS_CREATE,
        json!({"username": "reception", "display_name": "پذیرش", "password": "recep-pass-1", "role": "receptionist"}),
        Some(owner),
    );
    t.login("reception", "recep-pass-1")
}

#[test]
fn migration_adds_photo_and_sex_columns() {
    let t = T::new();
    t.setup();
    let cols = |table: &str| -> Vec<String> {
        t.core
            .with_db(|o| {
                let mut stmt = o.conn.prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
                let names =
                    stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(names)
            })
            .unwrap()
    };
    assert!(cols("patient").contains(&"photo_sha256".to_string()));
    let doctor = cols("doctor");
    assert!(doctor.contains(&"sex".to_string()) && doctor.contains(&"photo_sha256".to_string()));
    // Only male/female (or nothing) can be stored.
    let bad = t.core.with_db(|o| {
        Ok(o.conn.execute(
            "INSERT INTO doctor(id, full_name, color, created_at, updated_at, sex) VALUES ('x', 'X', '#000000', 'n', 'n', 'other')",
            [],
        )?)
    });
    assert!(bad.is_err(), "the CHECK constraint refuses other values");
}

#[test]
fn doctor_sex_is_optional_and_editable() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let d = t.ok(m::DOCTORS_CREATE, json!({"full_name": "داکتر احمد", "color": "#0e7490"}), Some(&owner));
    assert!(d["sex"].is_null(), "unset = neutral picture");
    assert!(d["photo_id"].is_null());
    let f = t.ok(
        m::DOCTORS_CREATE,
        json!({"full_name": "داکتر زرغونه", "color": "#be185d", "sex": "female"}),
        Some(&owner),
    );
    assert_eq!(f["sex"], "female");
    let mut upd = json!({
        "id": d["id"], "version": d["version"], "full_name": "داکتر احمد", "color": "#0e7490",
        "status": "active", "sex": "male"
    });
    let d2 = t.ok(m::DOCTORS_UPDATE, upd.clone(), Some(&owner));
    assert_eq!(d2["sex"], "male");
    upd["version"] = d2["version"].clone();
    upd["sex"] = Value::Null;
    assert!(t.ok(m::DOCTORS_UPDATE, upd, Some(&owner))["sex"].is_null(), "can be cleared again");
    let listed = t.ok(m::DOCTORS_LIST, json!({}), Some(&owner));
    assert!(listed.as_array().unwrap().iter().any(|x| x["sex"] == "female"));
}

#[test]
fn patient_photo_is_stored_encrypted_small_and_served_only_when_in_use() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let female = crate::ids::seed_id("reference_item", "gender/female");
    let p = t.ok(
        m::PATIENTS_CREATE,
        json!({"full_name": "مریم احمدی", "gender_id": female, "date_of_birth": "2016-03-01"}),
        Some(&owner),
    );
    assert_eq!(p["sex"], "female", "sex comes from the gender list value");
    assert!(p["photo_id"].is_null());
    let pid = p["id"].as_str().unwrap().to_string();

    let with = t.ok(m::PATIENTS_SET_PHOTO, json!({"id": pid, "data_base64": png(1600, 1200)}), Some(&owner));
    let photo = with["photo_id"].as_str().expect("photo set").to_string();
    assert_eq!(with["version"], p["version"], "a photo does not invalidate an open edit form");

    // Served as a small JPEG; the file on disk is encrypted, not a plain image.
    let got = t.ok(m::PHOTOS_GET, json!({"id": photo}), Some(&owner));
    let url = got["data_url"].as_str().unwrap();
    let jpeg =
        data_encoding::BASE64.decode(url.trim_start_matches("data:image/jpeg;base64,").as_bytes()).unwrap();
    let img = image::load_from_memory(&jpeg).unwrap();
    assert_eq!((img.width(), img.height()), (256, 192));
    let file = t.core.config().attachments_dir().join(&photo[0..2]).join(format!("{photo}.bin"));
    let raw = std::fs::read(&file).expect("kept in the attachment store, which backups copy");
    assert!(raw.starts_with(b"AAT1"), "encrypted container, not a plain JPEG");
    assert_ne!(raw.len(), jpeg.len());

    // Avatars for lists that only know ids: sex, age and photo; unknown ids are skipped.
    let av = t.ok(m::PATIENTS_AVATARS, json!({"patient_ids": [pid, pid, "nope"]}), Some(&owner));
    let av = av.as_array().unwrap();
    assert_eq!(av.len(), 1);
    assert_eq!(av[0]["sex"], "female");
    assert_eq!(av[0]["photo_id"], photo.as_str());
    assert!(av[0]["age"].as_i64().unwrap() >= 10);
    let many: Vec<String> = (0..501).map(|i| i.to_string()).collect();
    assert_eq!(
        t.call(m::PATIENTS_AVATARS, json!({"patient_ids": many}), Some(&owner)),
        Err(ErrorCode::Validation)
    );

    // Not an image, or too big: refused, nothing changes.
    assert_eq!(
        t.call(
            m::PATIENTS_SET_PHOTO,
            json!({"id": pid, "data_base64": data_encoding::BASE64.encode(b"%PDF-1.4")}),
            Some(&owner)
        ),
        Err(ErrorCode::Validation)
    );
    let huge = data_encoding::BASE64.encode(&vec![0x89u8; crate::attachment::MAX_PHOTO_BYTES + 1]);
    assert_eq!(
        t.call(m::PATIENTS_SET_PHOTO, json!({"id": pid, "data_base64": huge}), Some(&owner)),
        Err(ErrorCode::Validation)
    );

    // Any other attachment hash is never served through photos.get.
    let xray = t.ok(
        m::ATTACHMENTS_UPLOAD,
        json!({"patient_id": pid, "kind": "xray", "file_name": "x.png", "data_base64": png(40, 40)}),
        Some(&owner),
    );
    let xray_hash: String = t
        .core
        .with_db(|o| {
            Ok(o.conn.query_row(
                "SELECT sha256 FROM patient_attachment WHERE id = ?1",
                [xray["id"].as_str().unwrap()],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(t.call(m::PHOTOS_GET, json!({"id": xray_hash}), Some(&owner)), Err(ErrorCode::NotFound));

    // Removing the photo: back to the default picture; the old hash is no longer served.
    let without = t.ok(m::PATIENTS_SET_PHOTO, json!({"id": pid}), Some(&owner));
    assert!(without["photo_id"].is_null());
    assert_eq!(t.call(m::PHOTOS_GET, json!({"id": photo}), Some(&owner)), Err(ErrorCode::NotFound));
    let actions = t.audit_actions(&owner);
    assert!(actions.iter().filter(|a| *a == "patient.set_photo").count() >= 2);
}

#[test]
fn photo_permissions() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let rec = reception(&t, &owner);
    let p = t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ali"}), Some(&owner));
    let d = t.ok(m::DOCTORS_CREATE, json!({"full_name": "Dr", "color": "#0e7490"}), Some(&owner));
    // Reception registers patients (and may take their photo) but does not manage doctors.
    let with = t.ok(m::PATIENTS_SET_PHOTO, json!({"id": p["id"], "data_base64": png(64, 64)}), Some(&rec));
    assert_eq!(
        t.call(m::DOCTORS_SET_PHOTO, json!({"id": d["id"], "data_base64": png(64, 64)}), Some(&rec)),
        Err(ErrorCode::Forbidden)
    );
    let doc = t.ok(m::DOCTORS_SET_PHOTO, json!({"id": d["id"], "data_base64": png(64, 64)}), Some(&owner));
    assert!(doc["photo_id"].is_string());
    // Both pictures are visible to whoever sees the calendar or the patients.
    assert!(t.call(m::PHOTOS_GET, json!({"id": with["photo_id"]}), Some(&rec)).is_ok());
    assert!(t.call(m::PHOTOS_GET, json!({"id": doc["photo_id"]}), Some(&rec)).is_ok());
    assert_eq!(t.call(m::PHOTOS_GET, json!({"id": "x"}), None), Err(ErrorCode::Unauthenticated));
}

#[test]
fn merge_keeps_a_photo() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let keep = t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad Khan"}), Some(&owner));
    let dupe =
        t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad Khan", "allow_duplicate": true}), Some(&owner));
    let dupe =
        t.ok(m::PATIENTS_SET_PHOTO, json!({"id": dupe["id"], "data_base64": png(80, 80)}), Some(&owner));
    t.ok(
        m::PATIENTS_MERGE,
        json!({"keep_id": keep["id"], "merge_id": dupe["id"], "merge_id_version": dupe["version"]}),
        Some(&owner),
    );
    let kept = t.ok(m::PATIENTS_GET, json!({"patient_id": keep["id"]}), Some(&owner));
    assert_eq!(kept["photo_id"], dupe["photo_id"], "the kept record takes the merged record's photo");

    // A kept record that has its own photo keeps it.
    let other =
        t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ahmad Khan", "allow_duplicate": true}), Some(&owner));
    let other =
        t.ok(m::PATIENTS_SET_PHOTO, json!({"id": other["id"], "data_base64": png(90, 70)}), Some(&owner));
    t.ok(
        m::PATIENTS_MERGE,
        json!({"keep_id": keep["id"], "merge_id": other["id"], "merge_id_version": other["version"]}),
        Some(&owner),
    );
    let kept = t.ok(m::PATIENTS_GET, json!({"patient_id": keep["id"]}), Some(&owner));
    assert_eq!(kept["photo_id"], dupe["photo_id"]);
}

#[test]
fn backup_copies_the_photo_file() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let p = t.ok(m::PATIENTS_CREATE, json!({"full_name": "Ali"}), Some(&owner));
    let p = t.ok(m::PATIENTS_SET_PHOTO, json!({"id": p["id"], "data_base64": png(64, 64)}), Some(&owner));
    let photo = p["photo_id"].as_str().unwrap();
    let dst = tempfile::tempdir().unwrap();
    let copied = crate::backup::sync_attachments(&t.core.config().attachments_dir(), dst.path()).unwrap();
    assert!(copied >= 1);
    assert!(dst.path().join(&photo[0..2]).join(format!("{photo}.bin")).exists());
}
