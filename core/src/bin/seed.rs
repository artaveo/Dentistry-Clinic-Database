//! Large test database generator (roadmap 1.2).
//!
//!   cargo run --release -p artaveo-core --bin artaveo-seed -- --data-dir /tmp/big --users 200 --audit 100000
//!
//! Creates (or opens) a clinic, then bulk-inserts synthetic users, audit
//! history and (`--patients N`) patients, printing timings for the queries
//! the app runs on them (Phase 3 NFR (OF-016): search under 200 ms, profile under 500 ms at 100 000 patients; `--enforce` fails on a miss).
//! Synthetic data only.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use artaveo_core::{Config, Core, Environment};
use artaveo_shared::{methods as m, RpcRequest, RpcResponse};
use serde_json::{json, Value};

fn arg(name: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| default.into())
}

fn call(core: &Core, method: &str, params: Value, token: Option<&str>) -> Value {
    match core.handle(RpcRequest { method: method.into(), params, token: token.map(Into::into) }) {
        RpcResponse::Ok { result } => result,
        RpcResponse::Error { error } => panic!("{method}: {error:?}"),
    }
}

fn main() {
    let dir = PathBuf::from(arg("--data-dir", ".artaveo-seed"));
    let users: usize = arg("--users", "200").parse().unwrap();
    let audits: usize = arg("--audit", "100000").parse().unwrap();
    let patients: usize = arg("--patients", "0").parse().unwrap();
    let enforce = std::env::args().any(|a| a == "--enforce");
    let core = Core::open(Config::new(Environment::Test, dir.clone())).expect("open");

    if !core.is_set_up() {
        let r = call(
            &core,
            m::APP_SETUP,
            json!({
                "clinic_name": "Seed Clinic", "owner_username": "owner", "owner_display_name": "Owner",
                "owner_password": "owner-password", "language": "fa"
            }),
            None,
        );
        println!("setup done; recovery key {}", r["recovery_key"]);
    }
    let login = call(&core, m::AUTH_LOGIN, json!({"username": "owner", "password": "owner-password"}), None);
    let token = login["token"].as_str().unwrap().to_string();

    let t = Instant::now();
    let roles = ["administrator", "receptionist", "doctor", "accountant", "assistant"];
    for i in 0..users {
        call(
            &core,
            m::USERS_CREATE,
            json!({
                "username": format!("seed.user{i:05}"), "display_name": format!("کاربر آزمایشی {i}"),
                "password": "seed-password", "role": roles[i % roles.len()]
            }),
            Some(&token),
        );
    }
    println!("{users} users (Argon2id each): {:?}", t.elapsed());

    let t = Instant::now();
    artaveo_core::seeds_bulk_audit(&core, audits).expect("audit seed");
    println!("{audits} audit rows: {:?}", t.elapsed());

    for (label, params) in [
        ("audit.list first page", json!({"limit": 50, "offset": 0})),
        ("audit.list deep page", json!({"limit": 50, "offset": audits / 2})),
    ] {
        let t = Instant::now();
        call(&core, m::AUDIT_LIST, params, Some(&token));
        println!("{label}: {:?}", t.elapsed());
    }
    let t = Instant::now();
    call(&core, m::USERS_LIST, json!({}), Some(&token));
    println!("users.list: {:?}", t.elapsed());
    let t = Instant::now();
    let b = call(&core, m::BACKUP_CREATE, json!({}), Some(&token));
    println!("backup.create ({} bytes, verified): {:?}", b["size_bytes"], t.elapsed());

    if patients > 0 {
        let appointments: usize = arg("--appointments", "0").parse().unwrap();
        let t = Instant::now();
        artaveo_core::seeds_bulk_patients(&core, patients, appointments).expect("patient seed");
        println!("{patients} patients, {appointments} appointments: {:?}", t.elapsed());
        measure_phase3_and_4(&core, &token, patients, enforce);
    }
}

/// One timed scenario: the median of several runs, so one slow scheduler tick cannot fail the gate.
struct Measure {
    label: &'static str,
    median: Duration,
    /// Roadmap NFR the scenario must meet; `None` = informational.
    limit: Option<Duration>,
}

fn median_of(runs: usize, mut f: impl FnMut()) -> Duration {
    f(); // warm-up: page cache, prepared statements
    let mut v: Vec<Duration> = (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .collect();
    v.sort();
    v[v.len() / 2]
}

/// Phase 3/4 NFR gate (OF-016): patient search under 200 ms and opening a patient profile under
/// 500 ms with `--patients 100000`. With `--enforce` a miss exits non-zero (CI fails); the timings go
/// to the run summary either way.
fn measure_phase3_and_4(core: &Core, token: &str, patients: usize, enforce: bool) {
    let names = ["احمد خان", "زرغونه", "کریم داد", "فاطمه", "نجیب الله", "صدیقه"];
    let search = Some(Duration::from_millis(200));
    let profile = Some(Duration::from_millis(500));
    let first = call(core, m::PATIENTS_LIST, json!({"limit": 1}), Some(token))["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut rows: Vec<Measure> = Vec::new();
    let mut add = |label: &'static str, limit: Option<Duration>, f: &mut dyn FnMut()| {
        rows.push(Measure { label, median: median_of(7, f), limit });
    };
    add("patients.list first page", search, &mut || {
        call(core, m::PATIENTS_LIST, json!({"limit": 50, "offset": 0}), Some(token));
    });
    add("patients.list search by name", search, &mut || {
        call(core, m::PATIENTS_LIST, json!({"query": names[0], "limit": 50}), Some(token));
    });
    add("patients.list search by name + number", search, &mut || {
        call(
            core,
            m::PATIENTS_LIST,
            json!({"query": format!("{} {}", names[2], patients / 2), "limit": 50}),
            Some(token),
        );
    });
    add("patients.list search by phone prefix", search, &mut || {
        call(core, m::PATIENTS_LIST, json!({"query": "0799999", "limit": 50}), Some(token));
    });
    add("patients.check_duplicate", search, &mut || {
        call(
            core,
            m::PATIENTS_CHECK_DUPLICATE,
            json!({"full_name": names[1], "phone": "0700000001"}),
            Some(token),
        );
    });
    add("patient profile open (patient + history + files + visits + recalls)", profile, &mut || {
        let p = json!({"patient_id": first});
        call(core, m::PATIENTS_GET, p.clone(), Some(token));
        call(core, m::MEDICAL_HISTORY_GET, p.clone(), Some(token));
        call(core, m::ATTACHMENTS_LIST, p, Some(token));
        call(core, m::APPOINTMENTS_LIST, json!({"patient_id": first}), Some(token));
        call(core, m::RECALLS_LIST, json!({"patient_id": first}), Some(token));
    });
    add("appointments.list one day", Some(Duration::from_millis(300)), &mut || {
        call(
            core,
            m::APPOINTMENTS_LIST,
            json!({"date_from": "2024-02-01", "date_to": "2024-02-01"}),
            Some(token),
        );
    });
    add("appointments.counts one month", Some(Duration::from_millis(300)), &mut || {
        call(
            core,
            m::APPOINTMENTS_COUNTS,
            json!({"date_from": "2024-02-01", "date_to": "2024-02-29"}),
            Some(token),
        );
    });
    add("recalls.list call list", Some(Duration::from_millis(300)), &mut || {
        call(core, m::RECALLS_LIST, json!({}), Some(token));
    });

    let mut table = String::from("| Scenario | Median (7 runs) | Limit | |\n|---|---|---|---|\n");
    let mut failed = Vec::new();
    for r in &rows {
        let ok = r.limit.is_none_or(|l| r.median <= l);
        let limit = r.limit.map_or("-".to_string(), |l| format!("{} ms", l.as_millis()));
        println!("{}: {:?} (limit {limit}) {}", r.label, r.median, if ok { "ok" } else { "TOO SLOW" });
        table.push_str(&format!(
            "| {} | {:.1} ms | {limit} | {} |\n",
            r.label,
            r.median.as_secs_f64() * 1000.0,
            if ok { "✅" } else { "❌" }
        ));
        if !ok {
            failed.push(format!(
                "{} took {:.0} ms (limit {})",
                r.label,
                r.median.as_secs_f64() * 1000.0,
                limit
            ));
        }
    }
    if let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(path) {
            let _ = writeln!(f, "### Performance gate — {patients} patients\n\n{table}");
        }
    }
    if !failed.is_empty() {
        for f in &failed {
            println!("::error title=Performance gate failed::{f}");
        }
        if enforce {
            std::process::exit(1);
        }
    }
}
