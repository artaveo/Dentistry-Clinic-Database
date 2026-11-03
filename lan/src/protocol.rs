use artaveo_shared::{LiveEvent, RpcRequest, RpcResponse};
use serde::{Deserialize, Serialize};

/// Every frame on the socket is one JSON object of this enum. `Rpc`/`RpcResult` carry the very
/// same envelope as Tauri IPC (ADR-02: one API contract, two transports).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Frame {
    PairStart { device_name: String, spake: String },
    PairReply { spake: String },
    PairConfirm { mac: String },
    PairOk { mac: String, device_id: String, token: String, server_name: String },
    /// `reason`: `wrong_code` | `closed` | `limit_reached`.
    PairFailed { reason: String, attempts_left: u32 },

    Auth { device_id: String, token: String },
    AuthOk { server_name: String },
    AuthFailed,

    Rpc { id: u64, request: RpcRequest },
    RpcResult { id: u64, response: RpcResponse },
    /// A change somewhere in the clinic: the screen reloads what it shows.
    Event { event: LiveEvent },
    /// Heartbeat: a silent connection is declared dead and re-established.
    Ping,
    Pong,
}

impl Frame {
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).expect("frame serialises")
    }
    pub fn from_text(s: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}
