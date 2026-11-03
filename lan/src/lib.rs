//! LAN mode transport (roadmap Phase 7, ADR-02 and ADR-18).
//!
//! ```text
//! Client PC                                Clinic server PC
//!   │  mDNS browse _artaveo._tcp ───────────▶ advertised service
//!   │  TLS (cert not yet trusted, fp captured)
//!   │  PairStart{spake} ───────────────────▶ 6-digit code shown on the server screen
//!   │  ◀──────────────────────── PairReply{spake}
//!   │  PairConfirm{mac(K,"client"‖fp)} ────▶ verified (≤5 attempts per code)
//!   │  ◀── PairOk{mac(K,"server"‖fp), device_id, token}
//!   │  pin fp; keep the token (DPAPI)
//!   │
//!   │  TLS (pinned fp) + Auth{device_id, token}
//!   │  Rpc{id, request} ◀──▶ RpcResult{id, response}   ;   ◀── Event (push)
//! ```
//!
//! SPAKE2 makes a 6-digit code safe against offline guessing: an active attacker gets exactly one
//! guess per attempt. Binding the TLS certificate fingerprint into the confirmation MACs defeats a
//! man in the middle. Every request is answered by [`artaveo_core::Core::handle_from`], the same
//! router as Tauri IPC, so a client has exactly the permissions of the signed-in user.

pub mod client;
pub mod discovery;
pub mod pairing;
pub mod protocol;
pub mod server;
pub mod service;
pub mod tls;

pub const SERVICE_TYPE: &str = "_artaveo._tcp.local.";
/// The first port tried; the next few are used when it is taken.
pub const DEFAULT_PORT: u16 = 47821;
