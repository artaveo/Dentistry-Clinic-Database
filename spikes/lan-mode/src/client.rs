use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use rustls::pki_types::ServerName;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::pairing;
use crate::protocol::Frame;
use crate::tls::ClinicVerifier;

/// What a client remembers after pairing (stored DPAPI-protected on Windows).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedServer {
    pub fingerprint: [u8; 32],
    pub device_id: String,
    pub token: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("wrong pairing code ({attempts_left} attempts left)")]
    WrongCode { attempts_left: u32 },
    #[error("pairing refused: {0}")]
    Refused(String),
    #[error("server failed key confirmation (possible man-in-the-middle)")]
    ServerNotAuthentic,
    #[error("not authorised by server")]
    AuthFailed,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

type Ws = WebSocketStream<TlsStream<TcpStream>>;

async fn open(addr: &str, verifier: &Arc<ClinicVerifier>) -> anyhow::Result<Ws> {
    let tcp = TcpStream::connect(addr).await?;
    tcp.set_nodelay(true)?;
    let tls = TlsConnector::from(verifier.client_config()?)
        .connect(ServerName::try_from("artaveo-server.local")?.to_owned(), tcp)
        .await?;
    let (ws, _) = tokio_tungstenite::client_async(format!("wss://{addr}/v1"), tls).await?;
    Ok(ws)
}

async fn roundtrip(ws: &mut Ws, f: Frame) -> anyhow::Result<Frame> {
    ws.send(Message::Text(f.to_text())).await?;
    loop {
        match ws.next().await {
            Some(Ok(Message::Text(t))) => return Frame::from_text(&t),
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(e.into()),
            None => anyhow::bail!("server closed connection"),
        }
    }
}

pub async fn pair(addr: &str, code: &str, device_name: &str) -> Result<PairedServer, ClientError> {
    let verifier = ClinicVerifier::capture();
    let mut ws = open(addr, &verifier).await?;
    let fp = verifier.seen_fingerprint().ok_or_else(|| anyhow::anyhow!("no server certificate"))?;

    let (state, a) = pairing::start_client(code);
    let b =
        match roundtrip(&mut ws, Frame::PairStart { device_name: device_name.into(), spake: hex::encode(a) })
            .await?
        {
            Frame::PairReply { spake } => spake,
            Frame::PairFailed { reason, .. } => return Err(ClientError::Refused(reason)),
            other => return Err(anyhow::anyhow!("unexpected {other:?}").into()),
        };
    let key =
        state.finish(&hex::decode(b).map_err(anyhow::Error::from)?).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mac = hex::encode(pairing::confirm_mac(&key, b"client", &fp));
    match roundtrip(&mut ws, Frame::PairConfirm { mac }).await? {
        Frame::PairOk { mac, device_id, token } => {
            let mac = hex::decode(mac).map_err(anyhow::Error::from)?;
            if !pairing::verify_mac(&key, b"server", &fp, &mac) {
                return Err(ClientError::ServerNotAuthentic);
            }
            Ok(PairedServer { fingerprint: fp, device_id, token })
        }
        Frame::PairFailed { attempts_left, .. } => Err(ClientError::WrongCode { attempts_left }),
        other => Err(anyhow::anyhow!("unexpected {other:?}").into()),
    }
}

/// Authenticated connection: request/response + server push events.
pub struct Connection {
    sink: tokio::sync::Mutex<SplitSink<Ws, Message>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Frame>>>>,
    next_id: AtomicU64,
    pub server_name: String,
}

pub async fn connect(
    addr: &str,
    paired: &PairedServer,
) -> Result<(Connection, mpsc::UnboundedReceiver<Frame>), ClientError> {
    let mut ws = open(addr, &ClinicVerifier::pinned(paired.fingerprint)).await?;
    let server_name = match roundtrip(
        &mut ws,
        Frame::Auth { device_id: paired.device_id.clone(), token: paired.token.clone() },
    )
    .await?
    {
        Frame::AuthOk { server_name } => server_name,
        _ => return Err(ClientError::AuthFailed),
    };
    let (sink, mut stream) = ws.split();
    let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Frame>>>> = Default::default();
    let (ev_tx, ev_rx) = mpsc::unbounded_channel();
    let p = pending.clone();
    tokio::spawn(async move {
        while let Some(Ok(Message::Text(t))) = stream.next().await {
            match Frame::from_text(&t) {
                Ok(f @ Frame::RpcResult { id, .. }) => {
                    if let Some(tx) = p.lock().unwrap().remove(&id) {
                        let _ = tx.send(f);
                    }
                }
                Ok(f @ Frame::Event { .. }) => {
                    let _ = ev_tx.send(f);
                }
                _ => {}
            }
        }
    });
    Ok((
        Connection { sink: tokio::sync::Mutex::new(sink), pending, next_id: AtomicU64::new(1), server_name },
        ev_rx,
    ))
}

impl Connection {
    pub async fn call(&self, method: &str, params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.sink
            .lock()
            .await
            .send(Message::Text(Frame::Rpc { id, method: method.into(), params }.to_text()))
            .await?;
        match rx.await? {
            Frame::RpcResult { ok: true, data, .. } => Ok(data),
            Frame::RpcResult { data, .. } => anyhow::bail!("rpc error: {data}"),
            _ => unreachable!(),
        }
    }
}
