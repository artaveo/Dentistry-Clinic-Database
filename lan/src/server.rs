use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use artaveo_core::lan::Runtime;
use artaveo_core::Core;
use artaveo_shared::{ErrorCode, RpcError, RpcResponse};
use futures_util::{SinkExt, StreamExt};
use mdns_sd::ServiceDaemon;
use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;

use crate::pairing;
use crate::protocol::Frame;
use crate::tls::ServerIdentity;

/// A connection that sends nothing for this long is dropped (clients ping every few seconds).
const IDLE: Duration = Duration::from_secs(45);

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub bind_ip: IpAddr,
    /// Ports tried in order; `0` asks the system for a free one (tests).
    pub ports: Vec<u16>,
    /// Announce the server with mDNS so clients find it without typing an address.
    pub advertise: bool,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            ports: (crate::DEFAULT_PORT..crate::DEFAULT_PORT + 10).collect(),
            advertise: true,
        }
    }
}

/// A running LAN server. [`stop`](Self::stop) closes the listener and the announcement.
pub struct LanServer {
    pub port: u16,
    task: JoinHandle<()>,
    mdns: Option<ServiceDaemon>,
}

/// The computer's own IPv4 addresses on the network (shown on the Network page).
pub fn local_addresses() -> Vec<String> {
    if_addrs::get_if_addrs()
        .map(|list| {
            list.into_iter()
                .filter(|i| !i.is_loopback())
                .filter_map(|i| match i.ip() {
                    IpAddr::V4(v4) if !v4.is_link_local() => Some(v4.to_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn instance_name() -> String {
    let n: String = artaveo_core::audit::computer_name()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("artaveo-{}", if n.is_empty() { "server" } else { n.as_str() })
}

impl LanServer {
    pub async fn start(
        core: Arc<Core>,
        identity: ServerIdentity,
        opts: ServerOptions,
    ) -> anyhow::Result<Self> {
        let mut bound = None;
        for port in &opts.ports {
            match TcpListener::bind(SocketAddr::new(opts.bind_ip, *port)).await {
                Ok(l) => {
                    bound = Some(l);
                    break;
                }
                Err(e) => tracing::warn!(port, error = %e, "LAN port busy"),
            }
        }
        let listener = bound.ok_or_else(|| anyhow::anyhow!("no free LAN port"))?;
        let port = listener.local_addr()?.port();
        let fp = identity.fingerprint();
        let acceptor = TlsAcceptor::from(identity.server_config()?);
        let name = core.lan_server_name();
        let mdns = if opts.advertise {
            match crate::discovery::advertise(&name, &instance_name(), port, &fp) {
                Ok(d) => Some(d),
                // Typing the address still works (roadmap 7.2).
                Err(e) => {
                    tracing::warn!(error = %e, "mDNS announcement failed");
                    None
                }
            }
        } else {
            None
        };
        core.lan().set_runtime(Runtime { running: true, port: Some(port), addresses: local_addresses() });
        tracing::info!(port, "LAN server listening");

        let task = tokio::spawn(async move {
            loop {
                let Ok((tcp, peer)) = listener.accept().await else { continue };
                let _ = tcp.set_nodelay(true);
                let acceptor = acceptor.clone();
                let core = core.clone();
                tokio::spawn(async move {
                    let Ok(tls) = acceptor.accept(tcp).await else { return };
                    let Ok(ws) = tokio_tungstenite::accept_async(tls).await else { return };
                    if let Err(e) = session(core, fp, ws, peer).await {
                        tracing::debug!(error = %e, "LAN connection ended");
                    }
                });
            }
        });
        Ok(Self { port, task, mdns })
    }

    pub fn stop(self, core: &Core) {
        self.task.abort();
        if let Some(m) = &self.mdns {
            let _ = m.shutdown();
        }
        core.lan().set_runtime(Runtime::default());
    }
}

fn token_hash(token: &[u8]) -> String {
    hex::encode(Sha256::digest(token))
}

/// Counts a connection of a paired computer for as long as it lives.
struct Connected {
    core: Arc<Core>,
    device_id: String,
}

impl Drop for Connected {
    fn drop(&mut self) {
        self.core.lan().connection_closed(&self.device_id);
    }
}

fn unauthenticated() -> RpcResponse {
    RpcResponse::Error {
        error: RpcError {
            code: ErrorCode::Unauthenticated,
            detail: "this connection is not authorised".into(),
            field: None,
            rule: None,
        },
    }
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_rustls::server::TlsStream<TcpStream>>;

async fn session(core: Arc<Core>, fp: [u8; 32], ws: Ws, peer: SocketAddr) -> anyhow::Result<()> {
    let (mut tx, mut rx) = ws.split();
    let mut spake: Option<Vec<u8>> = None;
    let mut pending_name = String::new();
    let mut device: Option<(String, String)> = None; // (device id, device name)
    let mut _guard: Option<Connected> = None;
    let mut events = core.lan().subscribe();

    loop {
        tokio::select! {
            msg = tokio::time::timeout(IDLE, rx.next()) => {
                let msg = match msg { Ok(Some(Ok(m))) => m, _ => return Ok(()) };
                let text = match msg {
                    Message::Text(t) => t,
                    Message::Ping(p) => { tx.send(Message::Pong(p)).await?; continue }
                    Message::Close(_) => return Ok(()),
                    _ => continue,
                };
                let reply = match Frame::from_text(&text)? {
                    Frame::PairStart { device_name, spake: a } => match core.lan().pairing_code() {
                        None => Frame::PairFailed { reason: "closed".into(), attempts_left: 0 },
                        Some(code) => {
                            let (s, b) = pairing::start_server(&code);
                            spake = Some(s.finish(&hex::decode(a)?).map_err(|e| anyhow::anyhow!("{e:?}"))?);
                            pending_name = device_name;
                            Frame::PairReply { spake: hex::encode(b) }
                        }
                    },
                    Frame::PairConfirm { mac } => {
                        let Some(key) = spake.take() else { return Ok(()) };
                        if pairing::verify_mac(&key, b"client", &fp, &hex::decode(mac)?) {
                            let mut token = [0u8; 32];
                            rand::rngs::OsRng.fill_bytes(&mut token);
                            let (c, name, hash) = (core.clone(), pending_name.clone(), token_hash(&token));
                            let registered =
                                tokio::task::spawn_blocking(move || c.lan_register_device(&name, &hash)).await?;
                            match registered {
                                Ok(device_id) => {
                                    core.lan().pairing_done();
                                    Frame::PairOk {
                                        mac: hex::encode(pairing::confirm_mac(&key, b"server", &fp)),
                                        device_id,
                                        token: hex::encode(token),
                                        server_name: core.lan_server_name(),
                                    }
                                }
                                Err(e) if e.code() == ErrorCode::Forbidden => {
                                    Frame::PairFailed { reason: "limit_reached".into(), attempts_left: 0 }
                                }
                                Err(e) => return Err(e.into()),
                            }
                        } else {
                            Frame::PairFailed { reason: "wrong_code".into(), attempts_left: core.lan().pairing_failed() }
                        }
                    }
                    Frame::Auth { device_id, token } => {
                        let hash = token_hash(&hex::decode(token)?);
                        let (c, id, addr) = (core.clone(), device_id.clone(), peer.ip().to_string());
                        let name =
                            tokio::task::spawn_blocking(move || c.lan_authenticate(&id, &hash, &addr)).await??;
                        match name {
                            Some(name) => {
                                core.lan().connection_opened(&device_id);
                                _guard = Some(Connected { core: core.clone(), device_id: device_id.clone() });
                                device = Some((device_id, name));
                                Frame::AuthOk { server_name: core.lan_server_name() }
                            }
                            None => Frame::AuthFailed,
                        }
                    }
                    Frame::Ping if device.is_some() => Frame::Pong,
                    Frame::Rpc { id, request } => {
                        let Some((device_id, device_name)) = device.clone() else {
                            let r = Frame::RpcResult { id, response: unauthenticated() };
                            tx.send(Message::Text(r.to_text())).await?;
                            continue;
                        };
                        let c = core.clone();
                        let is_login = request.method == artaveo_shared::methods::AUTH_LOGIN;
                        let response = tokio::task::spawn_blocking(move || {
                            if !c.lan_device_active(&device_id) {
                                return None;
                            }
                            let r = c.handle_from(request, &device_name);
                            if is_login {
                                if let RpcResponse::Ok { result } = &r {
                                    if let Some(user) = result["user"]["display_name"].as_str() {
                                        c.lan_note_user(&device_id, user);
                                    }
                                }
                            }
                            Some(r)
                        })
                        .await?;
                        match response {
                            Some(response) => Frame::RpcResult { id, response },
                            // Revoked while connected.
                            None => return Ok(()),
                        }
                    }
                    _ => continue,
                };
                tx.send(Message::Text(reply.to_text())).await?;
            }
            ev = events.recv(), if device.is_some() => {
                match ev {
                    Ok(event) => {
                        if event.topic == "lan" {
                            // Server mode may have been turned off or this computer revoked: leave.
                            let (c, id) = (core.clone(), device.as_ref().map(|d| d.0.clone()).unwrap_or_default());
                            if !tokio::task::spawn_blocking(move || c.lan_enabled() && c.lan_device_active(&id)).await? {
                                return Ok(());
                            }
                        }
                        tx.send(Message::Text(Frame::Event { event }.to_text())).await?;
                    }
                    // Missed some: the screen reloads on the next event anyway.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => return Ok(()),
                }
            }
        }
    }
}
