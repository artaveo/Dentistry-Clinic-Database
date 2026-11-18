//! Database size budgets (docs/design/db-size-rules.md) and the measuring tool behind them.
//!
//! `dbsize_report` is `#[ignore]`d: it is a measuring tool, run on CI with
//! `cargo test -p artaveo-core dbsize_report -- --ignored --nocapture`. It prints GitHub `::notice`
//! annotations (readable on the public run page) and plain lines. The other tests are the budgets.

use std::path::{Path, PathBuf};

use artaveo_shared::methods as m;
use rusqlite::types::Value as SqlValue;
use rusqlite::Connection;
use serde_json::{json, Value};

use crate::db;
use crate::keys::DataKey;
use crate::tests::T;

fn pragma(conn: &Connection, name: &str) -> String {
    conn.query_row(&format!("PRAGMA {name}"), [], |r| r.get::<_, SqlValue>(0))
        .map(|v| match v {
            SqlValue::Integer(i) => i.to_string(),
            SqlValue::Text(s) => s,
            other => format!("{other:?}"),
        })
        .unwrap_or_else(|e| format!("err:{e}"))
}

fn pi(conn: &Connection, name: &str) -> i64 {
    pragma(conn, name).parse().unwrap_or(-1)
}

fn fsz(p: &Path) -> i64 {
    std::fs::metadata(p).map(|m| m.len() as i64).unwrap_or(0)
}

fn wal(p: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", p.display()))
}

fn kb(b: i64) -> String {
    format!("{:.1}KB", b as f64 / 1024.0)
}

/// One-line state of a database file.
fn state(conn: &Connection, path: &Path) -> String {
    format!(
        "file={} wal={} page_size={} cipher_page_size={} page_count={} freelist={} auto_vacuum={} journal_size_limit={} wal_autocheckpoint={}",
        kb(fsz(path)),
        kb(fsz(&wal(path))),
        pragma(conn, "page_size"),
        pragma(conn, "cipher_page_size"),
        pi(conn, "page_count"),
        pi(conn, "freelist_count"),
        pragma(conn, "auto_vacuum"),
        pragma(conn, "journal_size_limit"),
        pragma(conn, "wal_autocheckpoint"),
    )
}

fn schema_counts(conn: &Connection) -> String {
    let q = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(-1);
    format!(
        "tables={} (virtual={}) explicit_indexes={} auto_indexes={} triggers={} btrees(rootpage>0)={}",
        q("SELECT count(*) FROM sqlite_master WHERE type='table'"),
        q("SELECT count(*) FROM sqlite_master WHERE type='table' AND sql LIKE 'CREATE VIRTUAL%'"),
        q("SELECT count(*) FROM sqlite_master WHERE type='index' AND sql IS NOT NULL"),
        q("SELECT count(*) FROM sqlite_master WHERE type='index' AND sql IS NULL"),
        q("SELECT count(*) FROM sqlite_master WHERE type='trigger'"),
        q("SELECT count(*) FROM sqlite_master WHERE rootpage > 0"),
    )
}

struct Obj {
    name: String,
    table: String,
    kind: String,
    pages: i64,
    payload: Option<i64>,
}

fn master(conn: &Connection) -> Vec<(String, String, String, Option<String>)> {
    let mut st = conn.prepare("SELECT type, name, tbl_name, sql FROM sqlite_master").unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// Pages per b-tree: `dbstat` when compiled in, otherwise freelist deltas of DROPs on a copy.
fn breakdown(conn: &Connection, key: &DataKey, scratch: &Path) -> (String, Vec<Obj>) {
    let objs = master(conn);
    let kind_of = |name: &str| -> (String, String) {
        objs.iter()
            .find(|o| o.1 == name)
            .map(|o| (o.0.clone(), o.2.clone()))
            .unwrap_or(("schema".into(), name.into()))
    };
    let dbstat: rusqlite::Result<Vec<(String, i64, i64)>> = conn
        .prepare("SELECT name, count(*), sum(payload) FROM dbstat GROUP BY name")
        .and_then(|mut st| st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect());
    if let Ok(rows) = dbstat {
        let out = rows
            .into_iter()
            .map(|(name, pages, payload)| {
                let (kind, table) = kind_of(&name);
                Obj { name, table, kind, pages, payload: Some(payload) }
            })
            .collect();
        return ("dbstat".into(), out);
    }
    let copy = scratch.join(format!("breakdown-{}.db", crate::ids::new_id()));
    db::vacuum_into(conn, &copy).unwrap();
    let c = db::connect(&copy, key).unwrap();
    c.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
    let mut out = Vec::new();
    let mut drop_measure = |sql: String, name: &str, kind: &str, table: &str| {
        let f0 = pi(&c, "freelist_count");
        if c.execute_batch(&sql).is_ok() {
            out.push(Obj {
                name: name.into(),
                table: table.into(),
                kind: kind.into(),
                pages: pi(&c, "freelist_count") - f0,
                payload: None,
            });
        }
    };
    for (t, name, tbl, sql) in &objs {
        if t == "index" && sql.is_some() {
            drop_measure(format!("DROP INDEX \"{name}\""), name, "index", tbl);
        }
    }
    let virtuals: Vec<String> = objs
        .iter()
        .filter(|o| o.0 == "table" && o.3.as_deref().is_some_and(|s| s.starts_with("CREATE VIRTUAL")))
        .map(|o| o.1.clone())
        .collect();
    for v in &virtuals {
        drop_measure(format!("DROP TABLE \"{v}\""), v, "virtual+shadow", v);
    }
    for (t, name, _, sql) in &objs {
        let shadow = virtuals.iter().any(|v| name.starts_with(&format!("{v}_")));
        if t == "table"
            && !name.starts_with("sqlite_")
            && !shadow
            && !sql.as_deref().is_some_and(|s| s.starts_with("CREATE VIRTUAL"))
        {
            drop_measure(format!("DROP TABLE \"{name}\""), name, "table+autoidx", name);
        }
    }
    out.push(Obj {
        name: "(schema+left)".into(),
        table: "(schema)".into(),
        kind: "schema".into(),
        pages: pi(&c, "page_count") - pi(&c, "freelist_count"),
        payload: None,
    });
    drop(c);
    let _ = std::fs::remove_file(&copy);
    ("drop-delta".into(), out)
}

fn row_counts(conn: &Connection) -> Vec<(String, i64)> {
    master(conn)
        .into_iter()
        .filter(|o| o.0 == "table" && !o.1.starts_with("sqlite_"))
        .map(|o| {
            let n =
                conn.query_row(&format!("SELECT count(*) FROM \"{}\"", o.1), [], |r| r.get(0)).unwrap_or(-1);
            (o.1, n)
        })
        .collect()
}

/// Prints one GitHub notice (multi-line via %0A) and the same text in plain form.
fn notice(title: &str, lines: &[String]) {
    println!("==== {title}");
    for l in lines {
        println!("{l}");
    }
    let body = lines.join("%0A");
    println!("::notice title=dbsize {title}::{body}");
}

fn report_breakdown(title: &str, conn: &Connection, key: &DataKey, scratch: &Path, top: usize) {
    let page = pi(conn, "page_size").max(1);
    let (method, mut objs) = breakdown(conn, key, scratch);
    let rows = row_counts(conn);
    let rows_of = |t: &str| rows.iter().find(|r| r.0 == t).map(|r| r.1).unwrap_or(-1);
    objs.sort_by_key(|o| -o.pages);
    let total: i64 = objs.iter().map(|o| o.pages).sum();
    let one_page = objs.iter().filter(|o| o.pages == 1).count();
    // Per table: the table and its indexes.
    let mut per: Vec<(String, i64, i64)> = Vec::new();
    for o in &objs {
        match per.iter_mut().find(|p| p.0 == o.table) {
            Some(p) => p.1 += o.pages,
            None => per.push((o.table.clone(), o.pages, rows_of(&o.table))),
        }
    }
    per.sort_by_key(|p| -p.1);
    let empty_pages: i64 = per.iter().filter(|p| p.2 == 0).map(|p| p.1).sum();
    let empty_tables = per.iter().filter(|p| p.2 == 0).count();
    let audit_pages: i64 = per.iter().filter(|p| p.0 == "audit_log").map(|p| p.1).sum();
    let fts_pages: i64 = objs.iter().filter(|o| o.name.starts_with("patient_fts")).map(|o| o.pages).sum();
    let mut lines = vec![
        format!(
            "method={method} page={page} btree_pages_total={total} ({}) objects={} one_page_objects={one_page}",
            kb(total * page),
            objs.len()
        ),
        format!(
            "empty tables (0 rows)={empty_tables} using {empty_pages} pages ({}); audit_log={} ; patient_fts={}",
            kb(empty_pages * page),
            kb(audit_pages * page),
            kb(fts_pages * page)
        ),
        "-- per table (incl. its indexes): pages bytes rows".into(),
    ];
    for p in per.iter().take(top) {
        lines.push(format!("{} {} {} rows={}", p.0, p.1, kb(p.1 * page), p.2));
    }
    lines.push("-- largest single b-trees: name kind pages payload".into());
    for o in objs.iter().take(top) {
        lines.push(format!("{} {} {} {}", o.name, o.kind, o.pages, o.payload.map(kb).unwrap_or_default()));
    }
    notice(title, &lines);
}

fn checkpoint(conn: &Connection) {
    db::checkpoint(conn).unwrap();
}

/// A connection with a chosen SQLCipher page size (must be set before the first read).
fn connect_ps(path: &Path, key: &DataKey, ps: i64) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(&format!(
        "PRAGMA key = {}; PRAGMA cipher_page_size = {ps};",
        key.sqlcipher_pragma_value()
    ))
    .unwrap();
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn
}

/// Size of a compact copy of the database written with another page size (same key).
fn export_size(conn: &Connection, ps: i64, scratch: &Path) -> i64 {
    let p = scratch.join(format!("exp-{}.db", crate::ids::new_id()));
    conn.execute("ATTACH DATABASE ?1 AS exp", [p.to_string_lossy()]).unwrap();
    conn.execute_batch(&format!("PRAGMA exp.cipher_page_size = {ps};")).unwrap();
    conn.query_row("SELECT sqlcipher_export('exp')", [], |_| Ok(())).unwrap();
    conn.execute_batch("DETACH DATABASE exp;").unwrap();
    let s = fsz(&p);
    let _ = std::fs::remove_file(&p);
    s
}

fn exports(conn: &Connection, scratch: &Path) -> String {
    [1024, 2048, 4096]
        .iter()
        .map(|ps| format!("{ps}={}", kb(export_size(conn, *ps, scratch))))
        .collect::<Vec<_>>()
        .join(" ")
}

fn vacuumed_size(conn: &Connection, scratch: &Path) -> i64 {
    let p = scratch.join(format!("vac-{}.db", crate::ids::new_id()));
    db::vacuum_into(conn, &p).unwrap();
    let s = fsz(&p);
    let _ = std::fs::remove_file(&p);
    s
}

/// Schema only, then + seeds, then + roles/search index, at several SQLCipher page sizes.
fn layers_by_page_size(scratch: &Path) {
    let mut lines =
        vec!["ps: migrations-only | +seeds | +roles+fts | vacuumed (bytes after TRUNCATE checkpoint)".into()];
    for ps in [1024_i64, 2048, 4096] {
        let dir = scratch.join(format!("ps{ps}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clinic.db");
        let key = DataKey::generate();
        let mut conn = connect_ps(&path, &key, ps);
        db::migrate(&mut conn, db::MIGRATIONS, &dir.join("b")).unwrap();
        checkpoint(&conn);
        let a = fsz(&path);
        let pages_a = pi(&conn, "page_count");
        crate::seeds::sync(&mut conn).unwrap();
        checkpoint(&conn);
        let b = fsz(&path);
        crate::auth::sync_roles(&mut conn).unwrap();
        crate::patient::ensure_search_index(&conn).unwrap();
        checkpoint(&conn);
        let c = fsz(&path);
        let v = vacuumed_size(&conn, &dir);
        lines.push(format!(
            "{ps}: {} ({pages_a} pages) | {} | {} | {}  freelist={}",
            kb(a),
            kb(b),
            kb(c),
            kb(v),
            pi(&conn, "freelist_count")
        ));
        if ps == db::PAGE_SIZE {
            drop(conn);
            let conn = db::connect(&path, &key).unwrap();
            lines.push(format!("{ps} schema: {}", schema_counts(&conn)));
        }
    }
    notice("layers by page size", &lines);
}

fn day(n: i64) -> String {
    let d = crate::scheduling::parse_date("d", &crate::clock::today_iso()).unwrap() + time::Duration::days(n);
    crate::scheduling::format_date(d)
}

fn id_of(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[test]
#[ignore = "measuring tool: run explicitly on CI"]
fn dbsize_report() {
    let scratch = tempfile::tempdir().unwrap();
    let scratch = scratch.path();
    layers_by_page_size(scratch);

    let t = T::new();
    let path = t.core.config().db_path();
    let snap = |label: &str| -> String {
        t.core.with_db(|o| Ok(format!("{label}: {}", state(&o.conn, &path)))).unwrap()
    };
    t.setup();
    let mut lines = vec![snap("after app.setup")];
    let owner = t.login("owner", "owner-pass-1");
    lines.push(snap("after login"));
    t.core
        .with_db(|o| {
            lines.push(format!("schema: {}", schema_counts(&o.conn)));
            lines.push(format!("vacuumed copy: {}", kb(vacuumed_size(&o.conn, scratch))));
            checkpoint(&o.conn);
            lines.push(format!("after checkpoint(TRUNCATE): {}", state(&o.conn, &path)));
            Ok(())
        })
        .unwrap();
    notice("fresh setup", &lines);
    t.core
        .with_db(|o| {
            report_breakdown("fresh breakdown", &o.conn, &o.key, scratch, 45);
            let rows = row_counts(&o.conn);
            let mut nonempty: Vec<_> = rows.iter().filter(|r| r.1 > 0).collect();
            nonempty.sort_by_key(|r| -r.1);
            let line: Vec<String> = nonempty.iter().map(|r| format!("{}={}", r.0, r.1)).collect();
            notice("fresh row counts", &[line.join(" ")]);
            Ok(())
        })
        .unwrap();

    // Growth through the real API (with audit rows, FTS, counters).
    let size_now = || -> (i64, i64, i64) {
        t.core
            .with_db(|o| {
                let before_wal = fsz(&wal(&path));
                checkpoint(&o.conn);
                Ok((
                    (pi(&o.conn, "page_count") - pi(&o.conn, "freelist_count")) * pi(&o.conn, "page_size"),
                    fsz(&path),
                    before_wal,
                ))
            })
            .unwrap()
    };
    let mut growth =
        vec!["step: used-bytes(page_count-freelist) file wal-before-checkpoint | delta used".into()];
    let mut last = size_now();
    growth.push(format!(
        "start: compact copy by page size {}",
        t.core.with_db(|o| Ok(exports(&o.conn, scratch))).unwrap()
    ));
    let mut step = |label: &str, growth: &mut Vec<String>| {
        let now = size_now();
        growth.push(format!("{label}: {} {} {} | +{}", kb(now.0), kb(now.1), kb(now.2), kb(now.0 - last.0)));
        growth.push(format!(
            "  compact copy by page size {}",
            t.core.with_db(|o| Ok(exports(&o.conn, scratch))).unwrap()
        ));
        last = now;
    };
    let male = t.ok(m::REFERENCE_LIST, json!({"type_code": "gender", "language": "fa"}), Some(&owner));
    let gender = male.as_array().and_then(|a| a.first()).map(|g| g["id"].clone()).unwrap_or(Value::Null);
    let mut patients = Vec::new();
    for i in 0..100 {
        let p = t.ok(
            m::PATIENTS_CREATE,
            json!({"full_name": format!("احمد کریمی {i}"), "father_name": "محمد", "phone": format!("0799{:06}", i),
                   "gender_id": gender, "approximate_age": 30, "address": "کابل، کارته سه",
                   "allow_duplicate": true}),
            Some(&owner),
        );
        patients.push(id_of(&p));
    }
    step("+100 patients (API)", &mut growth);
    let doctor =
        id_of(&t.ok(m::DOCTORS_CREATE, json!({"full_name": "دکتر احمد", "color": "#0E7490"}), Some(&owner)));
    for (i, p) in patients.iter().enumerate() {
        let start = 8 * 60 + (i % 8) as i64 * 30;
        t.ok(
            m::APPOINTMENTS_CREATE,
            json!({"patient_id": p, "doctor_id": doctor, "date": day(1 + i as i64 / 8),
                   "start_time": format!("{:02}:{:02}", start / 60, start % 60),
                   "end_time": format!("{:02}:{:02}", (start + 30) / 60, (start + 30) % 60),
                   "reason": "معاینه", "override_schedule": true}),
            Some(&owner),
        );
    }
    step("+100 appointments (API)", &mut growth);
    let service = t.ok(m::CATALOG_GET, json!({}), Some(&owner))["services"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["code"] == "SRG-01")
        .map(|s| s["id"].clone())
        .unwrap();
    let cash = t
        .ok(m::PAYMENT_METHODS_LIST, json!({}), Some(&owner))
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["code"] == "cash")
        .map(|x| x["id"].clone())
        .unwrap();
    for p in &patients {
        let item = t.ok(
            m::TREATMENT_ITEMS_ADD,
            json!({"patient_id": p, "service_id": service, "teeth": [36], "perform_now": true,
                   "doctor_id": doctor, "unit_price": 350_000}),
            Some(&owner),
        )[0]
        .clone();
        let inv = t.ok(
            m::INVOICES_CREATE,
            json!({"patient_id": p, "items": [{"item_id": item["id"]}]}),
            Some(&owner),
        );
        t.ok(
            m::PAYMENTS_ADD,
            json!({"patient_id": p, "invoice_id": inv["id"], "amount": 350_000, "method_id": cash}),
            Some(&owner),
        );
    }
    step("+100 x (treatment item + invoice + payment) (API)", &mut growth);
    notice("growth per 100", &growth);

    t.core
        .with_db(|o| {
            report_breakdown("after growth breakdown", &o.conn, &o.key, scratch, 30);
            let mut l = vec![format!("live: {}", state(&o.conn, &path))];
            l.push(format!("vacuumed copy: {}", kb(vacuumed_size(&o.conn, scratch))));
            let audit: (i64, i64) = o
                .conn
                .query_row(
                    "SELECT count(*), COALESCE(sum(length(CAST(old_value AS BLOB)) + length(CAST(new_value AS BLOB))), 0) FROM audit_log",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap_or((-1, -1));
            l.push(format!("audit_log rows={} json bytes={}", audit.0, kb(audit.1)));
            notice("after growth state", &l);
            Ok(())
        })
        .unwrap();

    // Backups.
    let b = t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    let mut bl = vec![format!("backup.create size_bytes={}", b["size_bytes"])];
    let dir = t.core.config().backup_dir();
    let mut walk = vec![dir.clone()];
    while let Some(d) = walk.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk.push(p);
            } else {
                bl.push(format!("{} {}", p.strip_prefix(&dir).unwrap().display(), kb(fsz(&p))));
            }
        }
    }
    notice("backup files", &bl);
}

// ───────────────────────────── budgets ─────────────────────────────

/// A new clinic (setup done, every seed row in) may take at most this much (it was 1524 KB with
/// 4096-byte pages and the province/district labels stored; measured 607 KB now).
const FRESH_BUDGET: i64 = 640 * 1024;
/// 100 patients registered through the API, with their audit rows and search index (measured
/// about 200 KB with 1024-byte pages; see `db::LARGE_PAGE_SIZE` for why a big clinic switches).
const PER_100_PATIENTS_BUDGET: i64 = 230 * 1024;

/// Bytes the database pages use (the main file without its free pages), after a checkpoint.
fn used_bytes(t: &T) -> i64 {
    t.core
        .with_db(|o| {
            db::checkpoint(&o.conn)?;
            Ok((pi(&o.conn, "page_count") - pi(&o.conn, "freelist_count")) * db::page_size(&o.conn)?)
        })
        .unwrap()
}

#[test]
fn a_new_clinic_database_uses_small_pages_and_stays_within_its_budget() {
    let t = T::new();
    t.setup();
    let path = t.core.config().db_path();
    // Setup checkpoints: the schema and the seed rows are in the file, not left in a 2 MB WAL.
    let wal_after_setup = fsz(&wal(&path));
    assert!(wal_after_setup < 128 * 1024, "WAL after setup {wal_after_setup} bytes");
    let page = t.core.with_db(|o| db::page_size(&o.conn)).unwrap();
    assert_eq!(page, db::PAGE_SIZE);
    let used = used_bytes(&t);
    let file = fsz(&path);
    println!("new clinic: {used} bytes used, file {file} bytes, page {page}");
    assert!(file <= FRESH_BUDGET, "a new clinic database is {file} bytes (budget {FRESH_BUDGET})");
    assert_eq!(used, file, "no free pages in a new database");
    // The province/district labels are served from the seed files, not stored.
    let geo: i64 = t
        .core
        .with_db(|o| {
            Ok(o.conn.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'geo_translation'",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(geo, 0);
    let wal_limit = t.core.with_db(|o| Ok(pi(&o.conn, "journal_size_limit"))).unwrap();
    assert_eq!(wal_limit, db::JOURNAL_SIZE_LIMIT);
}

#[test]
fn a_hundred_patients_grow_the_database_by_less_than_the_budget() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let before = used_bytes(&t);
    for i in 0..100 {
        t.ok(
            m::PATIENTS_CREATE,
            json!({"full_name": format!("احمد کریمی {i}"), "father_name": "محمد", "phone": format!("0799{i:06}"),
                   "approximate_age": 30, "address": "کابل، کارته سه", "allow_duplicate": true}),
            Some(&owner),
        );
    }
    let growth = used_bytes(&t) - before;
    println!("100 patients: +{growth} bytes");
    assert!(
        growth <= PER_100_PATIENTS_BUDGET,
        "100 patients took {growth} bytes (budget {PER_100_PATIENTS_BUDGET})"
    );
}

#[test]
fn vacuum_of_a_new_database_changes_nothing_and_is_idempotent() {
    let t = T::new();
    t.setup();
    let dir = tempfile::tempdir().unwrap();
    t.core
        .with_db(|o| {
            db::checkpoint(&o.conn)?;
            let live = fsz(&t.core.config().db_path());
            let a = dir.path().join("a.db");
            db::vacuum_into(&o.conn, &a)?;
            let copy = db::connect(&a, &o.key)?;
            assert_eq!(db::page_size(&copy)?, db::PAGE_SIZE, "a backup copy keeps the small pages");
            let b = dir.path().join("b.db");
            db::vacuum_into(&copy, &b)?;
            drop(copy);
            assert_eq!(fsz(&a), fsz(&b), "VACUUM twice gives the same size");
            assert!(fsz(&a) <= live, "the copy is never bigger than the live file");
            Ok(())
        })
        .unwrap();
}

#[test]
fn audit_values_are_stored_without_null_fields() {
    let v = json!({"a": 1, "b": null, "c": {"d": null, "e": "x"}, "f": [null, {"g": null}]});
    assert_eq!(crate::audit::compact_json(&v), r#"{"a":1,"c":{"e":"x"},"f":[null,{}]}"#);
}

#[test]
fn only_the_newest_pre_migration_copies_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    for (name, stamp) in [("v17-to-v18", 100), ("v18-to-v19", 300), ("v19-to-v20", 200), ("v16-to-v17", 50)] {
        std::fs::write(dir.path().join(format!("pre-migration-{name}-{stamp}.db")), b"x").unwrap();
    }
    std::fs::write(dir.path().join("artaveo-20260101-100000-daily.db"), b"x").unwrap();
    db::prune_pre_migration(dir.path(), 2);
    let mut left: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            "artaveo-20260101-100000-daily.db",
            "pre-migration-v18-to-v19-300.db",
            "pre-migration-v19-to-v20-200.db"
        ]
    );
}

/// Waits for the background integrity check that setup starts (its own connection keeps the file
/// open, and Windows cannot rename an open file), so a test can move the database around.
fn settled(t: T) -> T {
    for _ in 0..500 {
        if t.core.integrity().status != "running" {
            return t;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("the integrity check did not finish");
}

/// Rewrites `path` with another page size, the way a database made before v0.12.5 looks.
fn rewrite_with_page_size(path: &Path, key: &DataKey, ps: i64) {
    let conn = db::connect(path, key).unwrap();
    db::checkpoint(&conn).unwrap();
    let tmp = path.with_extension("rewrite");
    conn.execute(
        "ATTACH DATABASE ?1 AS other KEY ?2",
        rusqlite::params![tmp.to_string_lossy(), format!("x'{}'", hex::encode(key.as_bytes()))],
    )
    .unwrap();
    conn.execute_batch(&format!("PRAGMA other.cipher_page_size = {ps};")).unwrap();
    conn.query_row("SELECT sqlcipher_export('other')", [], |_| Ok(())).unwrap();
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    conn.execute_batch(&format!("PRAGMA other.user_version = {v}; DETACH DATABASE other;")).unwrap();
    drop(conn);
    for s in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{s}", path.display()));
    }
    std::fs::rename(&tmp, path).unwrap();
}

#[test]
fn a_database_with_the_old_4096_byte_pages_is_rewritten_smaller_and_keeps_everything() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    for i in 0..20 {
        t.ok(
            m::PATIENTS_CREATE,
            json!({"full_name": format!("بیمار {i}"), "phone": format!("07000000{i:02}")}),
            Some(&owner),
        );
    }
    let path = t.core.config().db_path();
    let key = t.core.with_db(|o| Ok(o.key.clone())).unwrap();
    let T { _dir: dir, core } = settled(t);
    drop(core);
    rewrite_with_page_size(&path, &key, db::LARGE_PAGE_SIZE);
    let legacy = db::connect(&path, &key).unwrap();
    assert_eq!(db::page_size(&legacy).unwrap(), db::LARGE_PAGE_SIZE, "the old layout still opens");
    drop(legacy);
    assert!(db::opens_with(&path, &key));
    let legacy_size = fsz(&path);

    let core =
        crate::Core::open(crate::Config::new(crate::Environment::Test, dir.path().to_path_buf())).unwrap();
    let t = T { _dir: dir, core };
    assert!(fsz(&path) < legacy_size, "{} bytes, was {legacy_size}", fsz(&path));
    assert_eq!(t.core.with_db(|o| db::page_size(&o.conn)).unwrap(), db::PAGE_SIZE);
    assert!(!path.with_extension("repage-old").exists() && !path.with_extension("repage").exists());
    let owner = t.login("owner", "owner-pass-1");
    let found = t.ok(m::PATIENTS_LIST, json!({"query": "بیمار", "limit": 50}), Some(&owner));
    assert_eq!(found["items"].as_array().unwrap().len(), 20, "data and the search index came along");
    t.core.with_db(|o| db::full_integrity_check(&o.conn)).unwrap();
}

#[test]
fn a_crash_between_the_two_renames_is_repaired_on_the_next_start() {
    let t = T::new();
    t.setup();
    let path = t.core.config().db_path();
    let key = t.core.with_db(|o| Ok(o.key.clone())).unwrap();
    let T { _dir: dir, core } = settled(t);
    drop(core);
    // The old file was moved aside and the new one never arrived.
    std::fs::rename(&path, path.with_extension("repage-old")).unwrap();
    assert!(!db::fit_page_size(&path, &key).unwrap(), "already small: nothing to rewrite");
    assert!(path.exists() && !path.with_extension("repage-old").exists());
    drop(dir);
}

#[test]
fn small_pages_until_the_measured_crossover_then_big_pages_with_a_gap_between() {
    use db::{target_page_size as target, GROW_AT_BYTES, LARGE_PAGE_SIZE, PAGE_SIZE, SHRINK_BELOW_BYTES};
    assert_eq!(target(PAGE_SIZE, 600 * 1024), PAGE_SIZE);
    assert_eq!(target(PAGE_SIZE, GROW_AT_BYTES - 1), PAGE_SIZE);
    assert_eq!(target(PAGE_SIZE, GROW_AT_BYTES), LARGE_PAGE_SIZE);
    assert_eq!(target(LARGE_PAGE_SIZE, 2 * 1024 * 1024), PAGE_SIZE, "an old v0.12.4 test database");
    assert_eq!(target(LARGE_PAGE_SIZE, SHRINK_BELOW_BYTES), LARGE_PAGE_SIZE);
    assert_eq!(target(LARGE_PAGE_SIZE, 100 * 1024 * 1024), LARGE_PAGE_SIZE);
    const { assert!(SHRINK_BELOW_BYTES < GROW_AT_BYTES, "no switching back and forth") };
}

#[test]
fn a_grown_clinic_is_rewritten_with_big_pages_and_backups_and_restore_still_work() {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    for i in 0..30 {
        t.ok(
            m::PATIENTS_CREATE,
            json!({"full_name": format!("رحیم {i}"), "allow_duplicate": true}),
            Some(&owner),
        );
    }
    let path = t.core.config().db_path();
    let key = t.core.with_db(|o| Ok(o.key.clone())).unwrap();
    let T { _dir: dir, core } = settled(t);
    drop(core);
    db::repage(&path, &key, db::LARGE_PAGE_SIZE).unwrap();
    // Opened as is (a small file with big pages would be shrunk at start, so open it directly).
    let conn = db::connect(&path, &key).unwrap();
    assert_eq!(db::page_size(&conn).unwrap(), db::LARGE_PAGE_SIZE);
    let patients: i64 = conn.query_row("SELECT count(*) FROM patient", [], |r| r.get(0)).unwrap();
    assert_eq!(patients, 30);
    // A backup copy keeps the big pages and opens again.
    let copy = dir.path().join("copy.db");
    db::vacuum_into(&conn, &copy).unwrap();
    drop(conn);
    let c = db::connect(&copy, &key).unwrap();
    assert_eq!(db::page_size(&c).unwrap(), db::LARGE_PAGE_SIZE);
    db::full_integrity_check(&c).unwrap();
    drop(c);
    // The next start sees a small database with big pages and gives it the small pages back.
    let core =
        crate::Core::open(crate::Config::new(crate::Environment::Test, dir.path().to_path_buf())).unwrap();
    let t = T { _dir: dir, core };
    assert_eq!(t.core.with_db(|o| db::page_size(&o.conn)).unwrap(), db::PAGE_SIZE);
    let owner = t.login("owner", "owner-pass-1");
    let found = t.ok(m::PATIENTS_LIST, json!({"query": "رحیم", "limit": 50}), Some(&owner));
    assert_eq!(found["items"].as_array().unwrap().len(), 30);
    let b = t.ok(m::BACKUP_CREATE, json!({}), Some(&owner));
    assert!(b["size_bytes"].as_i64().unwrap() > 0);
}
