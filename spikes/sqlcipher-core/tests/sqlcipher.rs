use key_protect::{default_protector, DataKey, RecoveryKey};
use rusqlite::params;
use sqlcipher_core::{Database, KeyFiles, Migration, StoreError, MIGRATIONS};

fn now() -> &'static str {
    "2026-10-01T08:30:00.000Z"
}

fn insert_patient(db: &Database, number: &str) -> String {
    let id = Database::new_id();
    db.conn
        .execute(
            "INSERT INTO patient(id, number, full_name, phone, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, number, "احمد ولی", "0700123456", now()],
        )
        .unwrap();
    id
}

#[test]
fn creates_encrypted_db_with_expected_pragmas() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clinic.db");
    let p = default_protector();
    let db = Database::create(&path, p.as_ref(), &RecoveryKey::generate()).unwrap();

    let v = db.sqlcipher_version().unwrap();
    assert!(v.starts_with('4'), "SQLCipher 4.x expected, got {v}");
    let jm: String = db.conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(jm, "wal");
    let fk: i64 = db.conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 1);

    insert_patient(&db, "P-000001");
    db.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    drop(db);

    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3"), "header must be encrypted");
    assert!(!bytes.windows("P-000001".len()).any(|w| w == b"P-000001"), "plaintext leaked into the file");
}

#[test]
fn wrong_key_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clinic.db");
    let p = default_protector();
    Database::create(&path, p.as_ref(), &RecoveryKey::generate()).unwrap();
    let err = Database::open_with_key(&path, &DataKey::generate()).err().unwrap();
    assert!(matches!(err, StoreError::WrongKey), "{err:?}");
}

#[test]
fn strict_and_foreign_keys_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clinic.db");
    let p = default_protector();
    let db = Database::create(&path, p.as_ref(), &RecoveryKey::generate()).unwrap();
    let pid = insert_patient(&db, "P-000001");

    // STRICT: a float/text into an INTEGER money column is refused (ADR-06).
    let r = db.conn.execute(
        "INSERT INTO invoice(id, patient_id, total_minor, issued_at, created_at, updated_at)
         VALUES (?1, ?2, 'abc', ?3, ?3, ?3)",
        params![Database::new_id(), pid, now()],
    );
    assert!(r.is_err(), "STRICT should reject text in INTEGER column");

    // FK: unknown patient refused.
    let r = db.conn.execute(
        "INSERT INTO invoice(id, patient_id, total_minor, issued_at, created_at, updated_at)
         VALUES (?1, 'missing', 150000, ?2, ?2, ?2)",
        params![Database::new_id(), now()],
    );
    assert!(r.is_err(), "FK should reject unknown patient");

    db.conn
        .execute(
            "INSERT INTO invoice(id, patient_id, total_minor, issued_at, created_at, updated_at)
             VALUES (?1, ?2, 150000, ?3, ?3, ?3)",
            params![Database::new_id(), pid, now()],
        )
        .unwrap();
}

#[test]
fn uuid_v7_ids_sort_by_creation_time() {
    let a = Database::new_id();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let b = Database::new_id();
    assert!(a < b);
    assert_eq!(&a[14..15], "7");
}

#[test]
fn reopen_with_os_protector_runs_integrity_checks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clinic.db");
    let p = default_protector();
    {
        let db = Database::create(&path, p.as_ref(), &RecoveryKey::generate()).unwrap();
        insert_patient(&db, "P-000001");
    }
    let db = Database::open(&path, p.as_ref()).unwrap();
    let n: i64 = db.conn.query_row("SELECT count(*) FROM patient", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

#[test]
fn restore_on_new_pc_with_recovery_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clinic.db");
    let p = default_protector();
    let rk = RecoveryKey::generate();
    let printed = rk.to_display();
    {
        let db = Database::create(&path, p.as_ref(), &rk).unwrap();
        insert_patient(&db, "P-000042");
    }
    // Simulate a new computer: the machine-bound key file does not exist.
    std::fs::remove_file(KeyFiles::beside(&path).local).unwrap();
    assert!(Database::open(&path, p.as_ref()).is_err());

    let typed = RecoveryKey::parse(&printed.to_lowercase()).unwrap();
    let db = Database::restore_with_recovery_key(&path, p.as_ref(), &typed).unwrap();
    let num: String = db.conn.query_row("SELECT number FROM patient", [], |r| r.get(0)).unwrap();
    assert_eq!(num, "P-000042");
    drop(db);
    // Local key file was re-created for this machine.
    Database::open(&path, p.as_ref()).unwrap();
}

#[test]
fn vacuum_into_backup_stays_encrypted_and_restorable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clinic.db");
    let p = default_protector();
    let db = Database::create(&path, p.as_ref(), &RecoveryKey::generate()).unwrap();
    insert_patient(&db, "P-000007");
    let bak = dir.path().join("clinic.bak");
    db.backup_to(&bak).unwrap();

    let bytes = std::fs::read(&bak).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3"), "backup must be encrypted");

    let key = p.unprotect(&std::fs::read(KeyFiles::beside(&path).local).unwrap()).unwrap();
    let restored = Database::open_with_key(&bak, &key).unwrap();
    restored.startup_checks().unwrap();
    let num: String = restored.conn.query_row("SELECT number FROM patient", [], |r| r.get(0)).unwrap();
    assert_eq!(num, "P-000007");
}

#[test]
fn migration_backup_drift_and_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clinic.db");
    let p = default_protector();
    let mut db = Database::create(&path, p.as_ref(), &RecoveryKey::generate()).unwrap();
    insert_patient(&db, "P-000001");

    // A new migration: backup is taken before applying.
    let mut set: Vec<Migration> =
        MIGRATIONS.iter().map(|m| Migration { version: m.version, name: m.name, sql: m.sql }).collect();
    set.push(Migration {
        version: 3,
        name: "patient_email",
        sql: "ALTER TABLE patient ADD COLUMN email TEXT;",
    });
    let backup = db.migrate(&set).unwrap().expect("backup before migration");
    assert!(backup.exists());
    let uv: i64 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(uv, 3);

    // A failing migration rolls back atomically.
    set.push(Migration {
        version: 4,
        name: "broken",
        sql: "CREATE TABLE ok_part(x INTEGER) STRICT; INSERT INTO nope VALUES (1);",
    });
    let err = db.migrate(&set).unwrap_err();
    assert!(matches!(err, StoreError::MigrationFailed { version: 4, .. }), "{err:?}");
    let exists: i64 = db
        .conn
        .query_row("SELECT count(*) FROM sqlite_master WHERE name='ok_part'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(exists, 0, "partial migration must be rolled back");
    set.pop();

    // Editing an applied migration is detected.
    set[2].sql = "ALTER TABLE patient ADD COLUMN email2 TEXT;";
    assert!(matches!(db.migrate(&set), Err(StoreError::MigrationDrift { version: 3 })));

    // Older app opening a newer DB refuses instead of corrupting it.
    assert!(matches!(db.migrate(MIGRATIONS), Err(StoreError::DatabaseNewerThanApp { db: 3, app: 2 })));
}
