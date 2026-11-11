//! Development / E2E transport: JSON-RPC over plain HTTP on 127.0.0.1.
//! Same envelope and router as Tauri IPC (ADR-02), so Playwright tests drive
//! the real Core. Never shipped (feature `dev-server`).
//!
//!   cargo run -p artaveo-core --features dev-server --bin artaveo-dev-server -- --data-dir .artaveo-e2e --port 8787
//!
//! The HTTP side is a few lines of std::net on purpose: one thread per connection, started the moment
//! the connection is accepted. tiny_http 0.12 (used before) hands each accepted connection to a thread
//! pool that decides whether to start a new thread by counting idle threads; a woken idle thread is still
//! counted until it takes a job, so when the browser opens several connections at once two jobs can be
//! queued for one idle thread. Every pool thread then stays inside a keep-alive connection, and the
//! queued connection is never read: its request (the header's clinic.get in the E2E suite) waits forever
//! while the server keeps answering on the other connections.

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use artaveo_core::{Config, Core, Environment};
use artaveo_shared::RpcRequest;

const CORS: &str = "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\n\
                    Access-Control-Allow-Methods: POST, OPTIONS\r\nAccess-Control-Max-Age: 600\r\n";

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

    fn note(&mut self, what: String) {
        self.done.push_back(what);
        if self.done.len() > 60 {
            self.done.pop_front();
        }
    }

    fn finish(&mut self, id: u64) {
        if let Some(i) = self.running.iter().position(|x| x.0 == id) {
            let (_, what, since) = self.running.remove(i);
            self.note(format!("{what} {}ms", since.elapsed().as_millis()));
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

/// One HTTP/1.1 request as the browser and Playwright send it (a Content-Length body, no chunking).
struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
    close: bool,
}

/// Reads the next request of a connection; `None` when the client closed it or sent something unreadable.
fn read_request(reader: &mut BufReader<TcpStream>) -> Option<HttpRequest> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut close = parts.next() == Some("HTTP/1.0");
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                length = value.parse().ok()?;
            } else if name.eq_ignore_ascii_case("connection") {
                close = value.eq_ignore_ascii_case("close");
            }
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(HttpRequest { method, path, body, close })
}

fn write_response(w: &mut TcpStream, status: &str, kind: &str, body: &str) -> io::Result<()> {
    let len = body.len();
    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {len}\r\n");
    w.write_all(format!("{head}{CORS}\r\n{body}").as_bytes())?;
    w.flush()
}

fn validation_error(e: &serde_json::Error) -> String {
    let detail = e.to_string();
    let error = serde_json::json!({"code": "validation", "detail": detail, "field": null, "rule": null});
    serde_json::json!({"status": "error", "error": error}).to_string()
}

/// Status, content type and body.
type Answer = (&'static str, &'static str, String);

fn answer(req: &HttpRequest, core: &Core, activity: &Mutex<Activity>, id: u64, port: u16) -> Answer {
    match (req.method.as_str(), req.path.as_str()) {
        ("OPTIONS", _) => ("200 OK", "text/plain", String::new()),
        ("POST", "/rpc") => {
            let out = match serde_json::from_slice::<RpcRequest>(&req.body) {
                Ok(r) => {
                    let method = r.method.clone();
                    lock(activity).rename(id, format!("{method} :{port} in core"));
                    let out = serde_json::to_string(&core.handle(r)).unwrap();
                    lock(activity).rename(id, format!("{method} :{port} writing"));
                    out
                }
                Err(e) => validation_error(&e),
            };
            ("200 OK", "application/json", out)
        }
        ("GET", "/health") => ("200 OK", "text/plain", "ok".into()),
        ("GET", "/inflight") => ("200 OK", "text/plain", lock(activity).report()),
        _ => ("404 Not Found", "text/plain", "not found".into()),
    }
}

/// Answers the requests of one connection in order until the client closes it (keep-alive).
fn serve_connection(stream: TcpStream, core: &Core, activity: &Mutex<Activity>) {
    let port = stream.peer_addr().map_or(0, |a| a.port());
    lock(activity).note(format!("accepted :{port}"));
    let _ = stream.set_nodelay(true);
    let Ok(mut writer) = stream.try_clone() else { return };
    let mut reader = BufReader::new(stream);
    while let Some(req) = read_request(&mut reader) {
        let id = lock(activity).start(format!("{} {} :{port}", req.method, req.path));
        let (status, kind, body) = answer(&req, core, activity, id, port);
        let written = write_response(&mut writer, status, kind, &body);
        lock(activity).finish(id);
        if written.is_err() || req.close {
            break;
        }
    }
    lock(activity).note(format!("closed :{port}"));
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
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
    println!("artaveo dev server on http://127.0.0.1:{port}/rpc");
    // Every accepted connection gets its own thread at once, so no connection can wait behind another
    // (see the note at the top of this file).
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let (core, activity) = (core.clone(), activity.clone());
        std::thread::spawn(move || serve_connection(stream, &core, &activity));
    }
}
