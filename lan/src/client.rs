//! The client side: pairing with a clinic server and a connection that stays up (roadmap 7.2).
//! A client computer has no database; every request travels to the server, which answers it with
//! the same Core router a single computer uses.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use artaveo_core::keys::{default_protector, DataKey};
use artaveo_shared::{ErrorCode, LiveEvent, RpcError, RpcRequest, RpcResponse};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use rustls::pki_types::ServerName;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, oneshot};
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::pairing;
use crate::protocol::Frame;
use crate::tls::ClinicVerifier;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const PING_EVERY: Duration = Duration::from_secs(8);
/// No frame (not even a pong) for this long: the server is gone.
const SILENCE: Duration = Duration::from_secs(25);
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
const RETRY_EVERY: Duration = Duration::from_secs(2);

/// What a client remembers after pairing.
#[derive(Debug, Clone)]
pub struct PairedServer {
    pub server_name: String,
    /// Last known `host:port`; found again through mDNS when it stops answering (DHCP changes).
    pub addr: String,
    pub fingerprint: [u8; 32],
    pub device_id: String,
    /// Hex of the 256-bit device token.
    pub token: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("wrong pairing code ({attempts_left} attempts left)")]
    WrongCode { attempts_left: u32 },
    #[error("pairing is not open on the server")]
    Closed,
    #[error("the plan's limit of computers is reached")]
    LimitReached,
    #[error("server failed key confirmation (possible man-in-the-middle)")]
    ServerNotAuthentic,
    #[error("this computer is not (or no longer) paired with the server")]
    NotPaired,
    #[error("the server cannot be reached: {0}")]
    Unreachable(String),
}

impl From<anyhow::Error> for ClientError {
    fn from(e: anyhow::Error) -> Self {
        ClientError::Unreachable(e.to_string())
    }
}

/// `host` or `host:port` → `host:port` (the default port is added).
pub fn normalize_addr(input: &str) -> String {
    let s = input.trim();
    if s.contains(':') && !s.ends_with(':') {
        s.to_string()
    } else {
        format!("{}:{}", s.trim_end_matches(':'), crate::DEFAULT_PORT)
    }
}

// ───────────────────────────── persistence ─────────────────────────────

#[derive(Serialize, Deserialize)]
struct StoredPairing {
    server_name: String,
    addr: String,
    fingerprint: String,
    device_id: String,
    /// The device token wrapped with this computer's key protection (Windows DPAPI).
    token_protected: String,
}

/// `lan-client.json` in the data folder; its presence makes this computer a client.
#[derive(Debug, Clone)]
pub struct ClientStore {
    path: PathBuf,
}

impl ClientStore {
    pub fn new(data_dir: &Path) -> Self {
        Self { path: data_dir.join("lan-client.json") }
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn load(&self) -> Option<PairedServer> {
        let stored: StoredPairing = serde_json::from_slice(&std::fs::read(&self.path).ok()?).ok()?;
        let key = default_protector().unprotect(&hex::decode(&stored.token_protected).ok()?).ok()?;
        let mut fingerprint = [0u8; 32];
        fingerprint.copy_from_slice(&hex::decode(&stored.fingerprint).ok().filter(|v| v.len() == 32)?);
        Some(PairedServer {
            server_name: stored.server_name,
            addr: stored.addr,
            fingerprint,
            device_id: stored.device_id,
            token: hex::encode(key.as_bytes()),
        })
    }

    pub fn save(&self, p: &PairedServer) -> anyhow::Result<()> {
        let mut token = [0u8; 32];
        token.copy_from_slice(&hex::decode(&p.token)?);
        let protected = default_protector().protect(&DataKey::from_bytes(token)).map_err(|e| anyhow::anyhow!("{e}"))?;
        let stored = StoredPairing {
            server_name: p.server_name.clone(),
            addr: p.addr.clone(),
            fingerprint: hex::encode(p.fingerprint),
            device_id: p.device_id.clone(),
            token_protected: hex::encode(protected),
        };
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.path, serde_json::to_vec_pretty(&stored)?)?;
        Ok(())
    }

    pub fn clear(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// ───────────────────────────── sockets ─────────────────────────────

type Ws = WebSocketStream<TlsStream<TcpStream>>;

async fn open(addr: &str, verifier: &Arc<ClinicVerifier>) -> anyhow::Result<Ws> {
    let attempt = async {
        let tcp = TcpStream::connect(addr).await?;
        tcp.set_nodelay(true)?;
        let tls = TlsConnector::from(verifier.client_config()?)
            .connect(ServerName::try_from("artaveo-server.local")?.to_owned(), tcp)
            .await?;
        let (ws, _) = tokio_tungstenite::client_async(format!("wss://{addr}/v1"), tls).await?;
        anyhow::Ok(ws)
    };
    tokio::time::timeout(CONNECT_TIMEOUT, attempt).await.map_err(|_| anyhow::anyhow!("timed out"))?
}

async fn roundtrip(ws: &mut Ws, f: Frame) -> anyhow::Result<Frame> {
    ws.send(Message::Text(f.to_text())).await?;
    loop {
        let next = tokio::time::timeout(CONNECT_TIMEOUT, ws.next())
            .await
            .map_err(|_| anyhow::anyhow!("the server did not answer"))?;
        match next {
            Some(Ok(Message::Text(t))) => return Frame::from_text(&t),
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(e.into()),
            None => anyhow::bail!("server closed the connection"),
        }
    }
}

/// Pairs this computer with the server that shows `code` (roadmap 7.2).
pub async fn pair(addr: &str, code: &str, device_name: &str) -> Result<PairedServer, ClientError> {
    let verifier = ClinicVerifier::capture();
    let mut ws = open(addr, &verifier).await?;
    let fp = verifier.seen_fingerprint().ok_or_else(|| anyhow::anyhow!("no server certificate"))?;

    let (state, a) = pairing::start_client(code);
    let b = match roundtrip(&mut ws, Frame::PairStart { device_name: device_name.into(), spake: hex::encode(a) })
        .await?
    {
        Frame::PairReply { spake } => spake,
        Frame::PairFailed { .. } => return Err(ClientError::Closed),
        other => return Err(anyhow::anyhow!("unexpected {other:?}").into()),
    };
    let key = state.finish(&hex::decode(b).map_err(anyhow::Error::from)?).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mac = hex::encode(pairing::confirm_mac(&key, b"client", &fp));
    match roundtrip(&mut ws, Frame::PairConfirm { mac }).await? {
        Frame::PairOk { mac, device_id, token, server_name } => {
            let mac = hex::decode(mac).map_err(anyhow::Error::from)?;
            if !pairing::verify_mac(&key, b"server", &fp, &mac) {
                return Err(ClientError::ServerNotAuthentic);
            }
            Ok(PairedServer { server_name, addr: addr.to_string(), fingerprint: fp, device_id, token })
        }
        Frame::PairFailed { reason, attempts_left } => Err(match reason.as_str() {
            "limit_reached" => ClientError::LimitReached,
            "closed" => ClientError::Closed,
            _ => ClientError::WrongCode { attempts_left },
        }),
        other => Err(anyhow::anyhow!("unexpected {other:?}").into()),
    }
}

/// Servers announcing themselves on the network (blocks for `timeout`).
pub async fn discover(timeout: Duration) -> Vec<crate::discovery::Found> {
    tokio::task::spawn_blocking(move || crate::discovery::browse(timeout).unwrap_or_default())
        .await
        .unwrap_or_default()
}

// ───────────────────────────── connection ─────────────────────────────

/// What the shell hears from the connection.
#[derive(Debug, Clone)]
pub enum Notice {
    Online,
    Offline,
    /// The server no longer accepts this computer (revoked): it must be paired again.
    Revoked,
    Live(LiveEvent),
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<RpcResponse>>>>;

struct Connection {
    sink: tokio::sync::Mutex<SplitSink<Ws, Message>>,
    pending: Pending,
    next_id: AtomicU64,
    alive: AtomicBool,
}

impl Connection {
    async fn call(&self, request: RpcRequest) -> Option<RpcResponse> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let sent = self.sink.lock().await.send(Message::Text(Frame::Rpc { id, request }.to_text())).await;
        if sent.is_err() {
            self.pending.lock().unwrap().remove(&id);
            return None;
        }
        rx.await.ok()
    }
}

fn unreachable_response() -> RpcResponse {
    RpcResponse::Error {
        error: RpcError {
            code: ErrorCode::ServerUnreachable,
            detail: "the clinic server computer cannot be reached".into(),
            field: None,
            rule: None,
        },
    }
}

/// Opens an authenticated connection. The returned receiver fires when the connection ends.
async fn establish(
    paired: &PairedServer,
    notices: broadcast::Sender<Notice>,
) -> Result<(Arc<Connection>, oneshot::Receiver<()>, String), ClientError> {
    let mut ws = open(&paired.addr, &ClinicVerifier::pinned(paired.fingerprint)).await?;
    let server_name = match roundtrip(
        &mut ws,
        Frame::Auth { device_id: paired.device_id.clone(), token: paired.token.clone() },
    )
    .await?
    {
        Frame::AuthOk { server_name } => server_name,
        Frame::AuthFailed => return Err(ClientError::NotPaired),
        other => return Err(anyhow::anyhow!("unexpected {other:?}").into()),
    };
    let (sink, mut stream) = ws.split();
    let conn = Arc::new(Connection {
        sink: tokio::sync::Mutex::new(sink),
        pending: Default::default(),
        next_id: AtomicU64::new(1),
        alive: AtomicBool::new(true),
    });
    let (closed_tx, closed_rx) = oneshot::channel();

    let reader = conn.clone();
    tokio::spawn(async move {
        loop {
            match tokio::time::timeout(SILENCE, stream.next()).await {
                Ok(Some(Ok(Message::Text(t)))) => match Frame::from_text(&t) {
                    Ok(Frame::RpcResult { id, response }) => {
                        if let Some(tx) = reader.pending.lock().unwrap().remove(&id) {
                            let _ = tx.send(response);
                        }
                    }
                    Ok(Frame::Event { event }) => {
                        let _ = notices.send(Notice::Live(event));
                    }
                    _ => {}
                },
                Ok(Some(Ok(Message::Close(_)))) | Ok(None) | Ok(Some(Err(_))) | Err(_) => break,
                Ok(Some(Ok(_))) => {}
            }
        }
        reader.alive.store(false, Ordering::Relaxed);
        // Callers waiting for an answer give up at once.
        reader.pending.lock().unwrap().clear();
        let _ = closed_tx.send(());
    });

    let pinger = conn.clone();
    tokio::spawn(async move {
        while pinger.alive.load(Ordering::Relaxed) {
            tokio::time::sleep(PING_EVERY).await;
            if pinger.sink.lock().await.send(Message::Text(Frame::Ping.to_text())).await.is_err() {
                break;
            }
        }
    });
    Ok((conn, closed_rx, server_name))
}

/// A server connection that reconnects by itself (roadmap 7.2). Requests made while it is down
/// are answered with `server_unreachable` so the screen can say "the clinic server is off".
pub struct LanClient {
    paired: Mutex<PairedServer>,
    store: Option<ClientStore>,
    conn: tokio::sync::RwLock<Option<Arc<Connection>>>,
    notices: broadcast::Sender<Notice>,
    online: AtomicBool,
}

impl LanClient {
    pub fn new(paired: PairedServer, store: Option<ClientStore>) -> Arc<Self> {
        Arc::new(Self {
            paired: Mutex::new(paired),
            store,
            conn: tokio::sync::RwLock::new(None),
            notices: broadcast::channel(256).0,
            online: AtomicBool::new(false),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Notice> {
        self.notices.subscribe()
    }

    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed)
    }

    pub fn server_name(&self) -> String {
        self.paired.lock().unwrap().server_name.clone()
    }

    pub fn address(&self) -> String {
        self.paired.lock().unwrap().addr.clone()
    }

    /// Starts the connect/reconnect loop.
    pub fn spawn(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(self.clone().run())
    }

    /// The connect/reconnect loop itself (ends only when the server revokes this computer).
    pub async fn run(self: Arc<Self>) {
        loop {
            let paired = self.paired.lock().unwrap().clone();
            match establish(&paired, self.notices.clone()).await {
                Ok((conn, closed, name)) => {
                    {
                        let mut p = self.paired.lock().unwrap();
                        if p.server_name != name {
                            p.server_name = name;
                        }
                    }
                    *self.conn.write().await = Some(conn);
                    self.online.store(true, Ordering::Relaxed);
                    let _ = self.notices.send(Notice::Online);
                    let _ = closed.await;
                    *self.conn.write().await = None;
                    self.online.store(false, Ordering::Relaxed);
                    let _ = self.notices.send(Notice::Offline);
                }
                Err(ClientError::NotPaired) => {
                    let _ = self.notices.send(Notice::Revoked);
                    return;
                }
                Err(e) => {
                    tracing::debug!(error = %e, addr = %paired.addr, "LAN server not reachable");
                    if self.online.swap(false, Ordering::Relaxed) {
                        let _ = self.notices.send(Notice::Offline);
                    }
                    self.rediscover(&paired).await;
                }
            }
            tokio::time::sleep(RETRY_EVERY).await;
        }
    }

    /// The server may have a new address (DHCP): look for it by its certificate fingerprint.
    async fn rediscover(&self, paired: &PairedServer) {
        let hint = hex::encode(&paired.fingerprint[..4]);
        let found = discover(Duration::from_secs(2)).await;
        if let Some(f) = found.into_iter().find(|f| f.fp_hint == hint) {
            if f.addr != paired.addr {
                let mut p = self.paired.lock().unwrap();
                p.addr = f.addr;
                if let Some(store) = &self.store {
                    let _ = store.save(&p);
                }
            }
        }
    }

    /// Sends one request to the server and waits for the answer.
    pub async fn call(&self, request: RpcRequest) -> RpcResponse {
        let conn = self.conn.read().await.clone();
        let Some(conn) = conn else { return unreachable_response() };
        match tokio::time::timeout(CALL_TIMEOUT, conn.call(request)).await {
            Ok(Some(response)) => response,
            _ => unreachable_response(),
        }
    }
}
