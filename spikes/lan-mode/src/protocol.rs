use serde::{Deserialize, Serialize};

/// Every frame on the socket is one JSON object of this enum.
/// The same `method`/`params` shapes are used by Tauri IPC in single-PC mode
/// (ADR-02: one API contract, two transports).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Frame {
    PairStart { device_name: String, spake: String },
    PairReply { spake: String },
    PairConfirm { mac: String },
    PairOk { mac: String, device_id: String, token: String },
    PairFailed { reason: String, attempts_left: u32 },

    Auth { device_id: String, token: String },
    AuthOk { server_name: String },
    AuthFailed,

    Rpc { id: u64, method: String, params: serde_json::Value },
    RpcResult { id: u64, ok: bool, data: serde_json::Value },
    Event { topic: String, data: serde_json::Value },
}

impl Frame {
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).expect("frame serialises")
    }
    pub fn from_text(s: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}
