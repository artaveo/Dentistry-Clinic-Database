//! Encryption overhead vs. NFRs: 100k patients, indexed search, backup.
//!   cargo run --release -p sqlcipher-core --example bench
use std::time::Instant;

use key_protect::{default_protector, RecoveryKey};
use rusqlite::params;
use sqlcipher_core::Database;

fn main() {
    let dir = std::env::temp_dir().join(format!("artaveo-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bench.db");
    let p = default_protector();
    let mut db = Database::create(&path, p.as_ref(), &RecoveryKey::generate()).unwrap();
    db.conn
        .execute_batch(
            "CREATE INDEX idx_patient_phone ON patient(phone) WHERE deleted_at IS NULL;
             CREATE INDEX idx_patient_name ON patient(full_name COLLATE NOCASE) WHERE deleted_at IS NULL;",
        )
        .unwrap();

    let n = 100_000;
    let t = Instant::now();
    let tx = db.conn.transaction().unwrap();
    {
        let mut st = tx
            .prepare(
                "INSERT INTO patient(id, number, full_name, phone, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            )
            .unwrap();
        for i in 0..n {
            st.execute(params![
                Database::new_id(),
                format!("P-{i:06}"),
                format!("مریض شماره {i} احمدزی"),
                format!("07{:08}", (i as u64 * 7919) % 100_000_000),
                "2026-10-01T08:30:00.000Z"
            ])
            .unwrap();
        }
    }
    tx.commit().unwrap();
    println!("insert {n} patients (1 tx): {:?}", t.elapsed());

    let t = Instant::now();
    let mut hits = 0;
    for i in 0..100 {
        // Prefix search as a range, so the BINARY index is used (LIKE would
        // need a NOCASE index; digits are already normalised per ADR-09).
        let lo = format!("07{:04}", (i * 37) % 10_000);
        let hi = format!("{lo}\u{10FFFF}");
        let mut st = db
            .conn
            .prepare_cached(
                "SELECT id, number, full_name FROM patient
                 WHERE deleted_at IS NULL AND phone >= ?1 AND phone < ?2 LIMIT 50",
            )
            .unwrap();
        hits += st.query_map([lo, hi], |_| Ok(())).unwrap().count();
    }
    println!("100 phone-prefix searches: {:?} total, {:?} avg ({hits} rows)", t.elapsed(), t.elapsed() / 100);

    let t = Instant::now();
    let row: String = db
        .conn
        .query_row("SELECT full_name FROM patient WHERE number = ?1", ["P-054321"], |r| r.get(0))
        .unwrap();
    println!("lookup by patient number: {:?} ({row})", t.elapsed());

    let t = Instant::now();
    db.backup_to(&dir.join("bench.bak")).unwrap();
    let size = std::fs::metadata(dir.join("bench.bak")).unwrap().len();
    println!("VACUUM INTO encrypted backup ({} MB): {:?}", size / 1_000_000, t.elapsed());

    for pragma in ["cipher_integrity_check", "quick_check", "foreign_key_check"] {
        let t = Instant::now();
        let mut st = db.conn.prepare(&format!("PRAGMA {pragma}")).unwrap();
        let rows = st.query_map([], |_| Ok(())).unwrap().count();
        println!("PRAGMA {pragma}: {:?} ({rows} rows)", t.elapsed());
    }
    drop(db);
    std::fs::remove_dir_all(dir).ok();
}
