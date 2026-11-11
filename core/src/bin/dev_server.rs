//! Development / E2E transport: JSON-RPC over plain HTTP on 127.0.0.1.
//! Same envelope and router as Tauri IPC (ADR-02), so Playwright tests drive
//! the real Core. Never shipped (feature `dev-server`).
//!
//!   cargo run -p artaveo-core --features dev-server --bin artaveo-dev-server -- --data-dir .artaveo-e2e --port 8787

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use artaveo_core::{Config, Core, Environment};
use artaveo_shared::RpcRequest;
use tiny_http::{Header, Method, Response, Server};

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

/// What the server is doing right now and what it finished lately, for the E2E suite to print when a
/// request stays unanswered (GET /inflight). Development only.
#[derive(Default)]
struct Activity {
    /// Id, what (method, client port, stage) and since when.
    running: Vec<(u64, String, Instant)>,
    done: VecDeque<String>,
    next: u64,
}

impl Activity {
    fn start(&mut self, what: String) -> u64 {
        self.next += 1;
        self.running.push((self.next, what, Instant::now()));
        self.next
    }

    fn rename(&mut self, id: u64, what: String) {
        if let Some(x) = self.running.iter_mut().find(|x| x.0 == id) {
            x.1 = what;
        }
    }

    fn finish(&mut self, id: u64) {
        if let Some(i) = self.running.iter().position(|x| x.0 == id) {
            let (_, what, since) = self.running.remove(i);
            self.done.push_back(format!("{what} {}ms", since.elapsed().as_millis()));
            if self.done.len() > 40 {
                self.done.pop_front();
            }
        }
    }

    fn report(&self) -> String {
        let ms = |x: &(u64, String, Instant)| format!("{} {}ms", x.1, x.2.elapsed().as_millis());
        let running: Vec<String> = self.running.iter().map(ms).collect();
        let done: Vec<&str> = self.done.iter().map(String::as_str).collect();
        format!("running: [{}] | done: [{}]", running.join(", "), done.join(", "))
    }
}

fn lock(activity: &Mutex<Activity>) -> MutexGuard<'_, Activity> {
    activity.lock().unwrap_or_else(|p| p.into_inner())
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

    // The scheduled backup holds the database for as long as it runs, so a request that arrives
    // meanwhile waits. In the E2E suite that made a request time out at a random moment (the rare
    // missing header status, roadmap phase 10 note); E2E tests make their backups through the API.
    if std::env::var_os("ARTAVEO_E2E_NO_TICK").is_none() {
        let tick = core.clone();
        std::thread::spawn(move || loop {
            if let Err(e) = tick.run_due_tasks() {
                tracing::error!(error = %e, "scheduled task failed");
            }
            std::thread::sleep(Duration::from_secs(300));
        });
    }

    let activity = Arc::new(Mutex::new(Activity::default()));
    let server = Server::http(("127.0.0.1", port)).expect("bind");
    println!("artaveo dev server on http://127.0.0.1:{port}/rpc");
    let cors = [
        Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap(),
        Header::from_bytes("Access-Control-Allow-Headers", "content-type").unwrap(),
        Header::from_bytes("Access-Control-Allow-Methods", "POST, OPTIONS").unwrap(),
    ];
    // One thread per request: the loop that accepts requests never waits for a slow one, so a request
    // that is still being read or answered cannot hold back the next (the E2E header flake: one
    // request of the page stayed unanswered while the others finished).
    for mut req in server.incoming_requests() {
        let (core, cors, activity) = (core.clone(), cors.clone(), activity.clone());
        std::thread::spawn(move || {
            let port = req.remote_addr().map_or(0, |a| a.port());
            let id = lock(&activity).start(format!("{} {} :{port} reading", req.method(), req.url()));
            let mut resp = match (req.method(), req.url()) {
                (Method::Options, _) => Response::from_string(""),
                (Method::Post, "/rpc") => {
                    let mut body = String::new();
                    let _ = req.as_reader().read_to_string(&mut body);
                    let out = match serde_json::from_str::<RpcRequest>(&body) {
                        Ok(r) => {
                            let method = r.method.clone();
                            lock(&activity).rename(id, format!("{method} :{port} in core"));
                            let out = serde_json::to_string(&core.handle(r)).unwrap();
                            lock(&activity).rename(id, format!("{method} :{port} writing"));
                            out
                        }
                        Err(e) => serde_json::json!({"status": "error", "error": {"code": "validation", "detail": e.to_string(), "field": null, "rule": null}}).to_string(),
                    };
                    Response::from_string(out)
                        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
                }
                (Method::Get, "/health") => Response::from_string("ok"),
                (Method::Get, "/inflight") => Response::from_string(lock(&activity).report()),
                _ => Response::from_string("not found").with_status_code(404),
            };
            for h in &cors {
                resp.add_header(h.clone());
            }
            let _ = req.respond(resp);
            lock(&activity).finish(id);
        });
    }
}
