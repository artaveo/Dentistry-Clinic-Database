use artaveo_shared::{LiveEvent, RpcRequest, RpcResponse};
use serde::{Deserialize, Serialize};

/// Every frame on the socket is one JSON object of this enum. `Rpc`/`RpcResult` carry the very
/// same envelope as Tauri IPC (ADR-02: one API contract, two transports).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Frame {
    PairStart {
        device_name: String,
        spake: String,
    },
    PairReply {
        spake: String,
    },
    PairConfirm {
        mac: String,
    },
    PairOk {
        mac: String,
        device_id: String,
        token: String,
        server_name: String,
    },
    /// `reason`: `wrong_code` | `closed` | `limit_reached`.
    PairFailed {
        reason: String,
        attempts_left: u32,
    },

    Auth {
        device_id: String,
        token: String,
        /// The program version of the client (roadmap 11.2). Empty from a client that predates it.
        #[serde(default)]
        version: String,
    },
    AuthOk {
        server_name: String,
    },
    AuthFailed,
    /// The server runs a version this client cannot work with: the client must be updated.
    Incompatible {
        server_version: String,
    },

    Rpc {
        id: u64,
        request: RpcRequest,
    },
    RpcResult {
        id: u64,
        response: RpcResponse,
    },
    /// A change somewhere in the clinic: the screen reloads what it shows.
    Event {
        event: LiveEvent,
    },
    /// Heartbeat: a silent connection is declared dead and re-established.
    Ping,
    Pong,
}

/// The version of the program both sides were built from.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Roadmap 11.2: in LAN mode the server and its clients must run compatible versions. Versions
/// are compatible when major and minor are equal (a patch release never changes what is sent). A
/// client that does not say its version is accepted: refusing it would lock out every computer
/// that was installed before this check existed.
pub fn compatible(client: &str, server: &str) -> bool {
    let minor = |v: &str| {
        let mut parts = v.split('.');
        (parts.next().map(str::to_string), parts.next().map(str::to_string))
    };
    client.is_empty() || minor(client) == minor(server)
}

impl Frame {
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).expect("frame serialises")
    }
    pub fn from_text(s: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_releases_work_together_but_other_minor_versions_do_not() {
        assert!(compatible("0.12.0", "0.12.3"));
        assert!(compatible("0.12.3", "0.12.0"));
        assert!(!compatible("0.11.1", "0.12.0"));
        assert!(!compatible("1.0.0", "0.12.0"));
    }

    #[test]
    fn a_client_that_does_not_say_its_version_is_accepted() {
        assert!(compatible("", "0.12.0"));
        let old = r#"{"type":"auth","device_id":"d","token":"t"}"#;
        match Frame::from_text(old).unwrap() {
            Frame::Auth { version, .. } => assert!(version.is_empty()),
            other => panic!("{other:?}"),
        }
    }
}
