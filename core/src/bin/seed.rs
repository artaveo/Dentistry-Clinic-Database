//! Large test database generator (roadmap 1.2).
//!
//!   cargo run --release -p artaveo-core --bin artaveo-seed -- --data-dir /tmp/big --users 200 --audit 100000
//!
//! Creates (or opens) a clinic, then bulk-inserts synthetic users, audit
//! history and (`--patients N`) patients, printing timings for the queries
//! the app runs on them (Phase 3 NFR: search under 200ms at 100 000 patients).
//! Synthetic data only.

use std::path::PathBuf;
use std::time::Instant;

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
        // Phase 3 NFR: patient search under 200ms with 100 000 patients.
        let names = ["احمد خان", "زرغونه", "کریم داد", "فاطمه", "نجیب الله", "صدیقه"];
        let t = Instant::now();
        for i in 0..patients {
            call(
                &core,
                m::PATIENTS_CREATE,
                json!({
                    "full_name": format!("{} {i}", names[i % names.len()]),
                    "phone": format!("07{:08}", i % 100_000_000),
                    "allow_duplicate": true
                }),
                Some(&token),
            );
        }
        println!("{patients} patients: {:?}", t.elapsed());

        for (label, params) in [
            ("patients.list first page", json!({"limit": 50, "offset": 0})),
            ("patients.list search by name", json!({"query": names[0], "limit": 50})),
            ("patients.list search by phone prefix", json!({"query": "0799999", "limit": 50})),
        ] {
            let t = Instant::now();
            call(&core, m::PATIENTS_LIST, params, Some(&token));
            println!("{label}: {:?}", t.elapsed());
        }
    }
}
