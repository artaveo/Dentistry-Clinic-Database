//! LAN Mode spike (ADR-02, Phase 7).
//!
//! ```text
//! Client                                   Server (clinic PC)
//!   │  mDNS browse _artaveo._tcp ───────────▶ advertised service
//!   │  TLS (cert not yet trusted, fp captured)
//!   │  ws: PairStart{spake_a} ──────────────▶ code shown on server screen
//!   │  ◀────────────────────── PairReply{spake_b}
//!   │  ws: PairConfirm{mac(K,"c"‖fp)} ──────▶ verify (≤5 attempts per code)
//!   │  ◀────── PairOk{mac(K,"s"‖fp), device_id, token}
//!   │  pin fp; store token
//!   │
//!   │  TLS (pinned fp) + ws: Auth{device_id, token}
//!   │  Rpc{id,method,params} ◀──▶ RpcResult   ;   ◀── Event{topic,data} (push)
//! ```
//!
//! SPAKE2 means a 6-digit code is safe against offline brute force: an
//! active attacker gets exactly one guess per attempt. Binding the TLS
//! certificate fingerprint into the confirmation MACs defeats a TLS MITM.

pub mod client;
pub mod discovery;
pub mod pairing;
pub mod protocol;
pub mod server;
pub mod tls;

pub const SERVICE_TYPE: &str = "_artaveo._tcp.local.";
