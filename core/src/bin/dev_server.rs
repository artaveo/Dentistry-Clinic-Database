//! Development / E2E transport: JSON-RPC over plain HTTP on 127.0.0.1.
//! Same envelope and router as Tauri IPC (ADR-02), so Playwright tests drive
//! the real Core. Never shipped (feature `dev-server`).
//!
//!   cargo run -p artaveo-core --features dev-server --bin artaveo-dev-server -- --data-dir .artaveo-e2e --port 8787

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use artaveo_core::{Config, Core, Environment};
use artaveo_shared::RpcRequest;
use tiny_http::{Header, Method, Response, Server};

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let data_dir = PathBuf::from(arg("--data-dir").unwrap_or_else(|| ".artaveo-development".into()));
    let port: u16 = arg("--port").and_then(|p| p.parse().ok()).unwrap_or(8787);
    let env = if std::env::var("ARTAVEO_ENV").as_deref() == Ok("test") {
        Environment::Test
    } else {
        Environment::Development
    };
    // Playwright runs the suite at a fixed clinic hour (ui/playwright.config.ts).
    if let Some(ms) = std::env::var("ARTAVEO_E2E_CLOCK_SHIFT_MS").ok().and_then(|v| v.parse().ok()) {
        artaveo_core::clock::set_shift_ms(ms);
        println!("clock shifted by {ms} ms");
    }
    let config = Config::new(env, data_dir);
    let _log = artaveo_core::logging::init(&config.log_dir(), env).expect("logging");
    let core = Arc::new(Core::open(config).expect("open core"));

    let tick = core.clone();
    std::thread::spawn(move || loop {
        if let Err(e) = tick.run_due_tasks() {
            tracing::error!(error = %e, "scheduled task failed");
        }
        std::thread::sleep(Duration::from_secs(300));
    });

    let server = Server::http(("127.0.0.1", port)).expect("bind");
    println!("artaveo dev server on http://127.0.0.1:{port}/rpc");
    let cors = [
        Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap(),
        Header::from_bytes("Access-Control-Allow-Headers", "content-type").unwrap(),
        Header::from_bytes("Access-Control-Allow-Methods", "POST, OPTIONS").unwrap(),
    ];
    for mut req in server.incoming_requests() {
        let mut resp = match (req.method(), req.url()) {
            (Method::Options, _) => Response::from_string(""),
            (Method::Post, "/rpc") => {
                let mut body = String::new();
                let _ = req.as_reader().read_to_string(&mut body);
                let out = match serde_json::from_str::<RpcRequest>(&body) {
                    Ok(r) => serde_json::to_string(&core.handle(r)).unwrap(),
                    Err(e) => serde_json::json!({"status": "error", "error": {"code": "validation", "detail": e.to_string(), "field": null, "rule": null}}).to_string(),
                };
                Response::from_string(out)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
            }
            (Method::Get, "/health") => Response::from_string("ok"),
            _ => Response::from_string("not found").with_status_code(404),
        };
        for h in &cors {
            resp.add_header(h.clone());
        }
        let _ = req.respond(resp);
    }
}
