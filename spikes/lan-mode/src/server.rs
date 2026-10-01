use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;

use crate::pairing::{self, MAX_ATTEMPTS};
use crate::protocol::Frame;
use crate::tls::ServerIdentity;

const CODE_TTL: Duration = Duration::from_secs(10 * 60);

struct PairingWindow {
    code: String,
    attempts_left: u32,
    issued: Instant,
}

struct State {
    pairing: Option<PairingWindow>,
    /// device_id → SHA-256(token). Tokens themselves are never stored.
    devices: HashMap<String, (String, [u8; 32])>,
}

#[derive(Clone)]
pub struct Server {
    pub name: String,
    identity: Arc<ServerIdentity>,
    state: Arc<Mutex<State>>,
    events: broadcast::Sender<Frame>,
}

impl Server {
    pub fn new(name: &str, identity: ServerIdentity) -> Self {
        Self {
            name: name.into(),
            identity: Arc::new(identity),
            state: Arc::new(Mutex::new(State { pairing: None, devices: HashMap::new() })),
            events: broadcast::channel(256).0,
        }
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        self.identity.fingerprint()
    }

    /// Owner clicks "Add computer": opens a pairing window and returns the code to display.
    pub fn open_pairing(&self) -> String {
        let code = pairing::new_code();
        self.state.lock().unwrap().pairing =
            Some(PairingWindow { code: code.clone(), attempts_left: MAX_ATTEMPTS, issued: Instant::now() });
        code
    }

    pub fn paired_devices(&self) -> Vec<String> {
        self.state.lock().unwrap().devices.values().map(|(n, _)| n.clone()).collect()
    }

    /// Real-time notification to all connected clients (e.g. "patient checked in").
    pub fn publish(&self, topic: &str, data: serde_json::Value) {
        let _ = self.events.send(Frame::Event { topic: topic.into(), data });
    }

    pub async fn serve(self, listener: TcpListener) -> anyhow::Result<()> {
        let acceptor = TlsAcceptor::from(self.identity.server_config()?);
        loop {
            let (tcp, _) = listener.accept().await?;
            tcp.set_nodelay(true)?;
            let acceptor = acceptor.clone();
            let this = self.clone();
            tokio::spawn(async move {
                let Ok(tls) = acceptor.accept(tcp).await else { return };
                let Ok(ws) = tokio_tungstenite::accept_async(tls).await else { return };
                let _ = this.session(ws).await;
            });
        }
    }

    async fn session<S>(&self, ws: tokio_tungstenite::WebSocketStream<S>) -> anyhow::Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let (mut tx, mut rx) = ws.split();
        let fp = self.fingerprint();
        let mut spake: Option<Vec<u8>> = None; // agreed key, pending confirmation
        let mut pending_name = String::new();
        let mut authed = false;
        let mut events = self.events.subscribe();

        loop {
            tokio::select! {
                msg = rx.next() => {
                    let Some(Ok(Message::Text(t))) = msg else { return Ok(()) };
                    let reply = match Frame::from_text(&t)? {
                        Frame::PairStart { device_name, spake: a } => {
                            let code = {
                                let mut st = self.state.lock().unwrap();
                                match &st.pairing {
                                    Some(w) if w.issued.elapsed() < CODE_TTL && w.attempts_left > 0 => Some(w.code.clone()),
                                    _ => { st.pairing = None; None }
                                }
                            };
                            match code {
                                None => Frame::PairFailed { reason: "pairing is not open on the server".into(), attempts_left: 0 },
                                Some(code) => {
                                    let (s, b) = pairing::start_server(&code);
                                    spake = Some(s.finish(&hex::decode(a)?).map_err(|e| anyhow::anyhow!("{e:?}"))?);
                                    pending_name = device_name;
                                    Frame::PairReply { spake: hex::encode(b) }
                                }
                            }
                        }
                        Frame::PairConfirm { mac } => {
                            let Some(key) = spake.take() else { return Ok(()) };
                            if pairing::verify_mac(&key, b"client", &fp, &hex::decode(mac)?) {
                                let mut token = [0u8; 32];
                                rand::rngs::OsRng.fill_bytes(&mut token);
                                let device_id = hex::encode(&token[..8]);
                                let mut st = self.state.lock().unwrap();
                                st.devices.insert(device_id.clone(), (pending_name.clone(), Sha256::digest(token).into()));
                                st.pairing = None; // one code, one computer
                                Frame::PairOk { mac: hex::encode(pairing::confirm_mac(&key, b"server", &fp)), device_id, token: hex::encode(token) }
                            } else {
                                let mut st = self.state.lock().unwrap();
                                let left = match st.pairing.as_mut() {
                                    Some(w) => { w.attempts_left = w.attempts_left.saturating_sub(1); w.attempts_left }
                                    None => 0,
                                };
                                if left == 0 { st.pairing = None; }
                                Frame::PairFailed { reason: "wrong code".into(), attempts_left: left }
                            }
                        }
                        Frame::Auth { device_id, token } => {
                            let hash: [u8; 32] = Sha256::digest(hex::decode(token)?).into();
                            let ok = self.state.lock().unwrap().devices.get(&device_id).is_some_and(|(_, h)| *h == hash);
                            if ok { authed = true; Frame::AuthOk { server_name: self.name.clone() } } else { Frame::AuthFailed }
                        }
                        Frame::Rpc { id, method, params } if authed => self.dispatch(id, &method, params),
                        Frame::Rpc { id, .. } => Frame::RpcResult { id, ok: false, data: "unauthenticated".into() },
                        _ => continue,
                    };
                    tx.send(Message::Text(reply.to_text())).await?;
                }
                ev = events.recv(), if authed => {
                    if let Ok(ev) = ev { tx.send(Message::Text(ev.to_text())).await?; }
                }
            }
        }
    }

    /// Stand-in for the Core command router shared with Tauri IPC.
    fn dispatch(&self, id: u64, method: &str, params: serde_json::Value) -> Frame {
        match method {
            "ping" => Frame::RpcResult { id, ok: true, data: params },
            "queue.check_in" => {
                self.publish("queue.changed", params.clone());
                Frame::RpcResult { id, ok: true, data: serde_json::json!({"status": "waiting"}) }
            }
            _ => Frame::RpcResult { id, ok: false, data: "unknown method".into() },
        }
    }
}
