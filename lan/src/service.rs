//! Keeps the LAN server in step with the "this computer is the clinic server" switch (roadmap 7.1).
//! The switch lives in the clinic database; this loop starts the listener when it is turned on and
//! stops it when it is turned off, so the Network page needs no restart.

use std::sync::Arc;
use std::time::Duration;

use artaveo_core::lan::Runtime;
use artaveo_core::Core;

use crate::server::{LanServer, ServerOptions};
use crate::tls::ServerIdentity;

/// The identity stored in the clinic database, created on first use (ADR-18: it is kept across
/// restarts and moves with a Backup, so paired computers keep trusting the server).
pub async fn load_or_create_identity(core: &Arc<Core>) -> anyhow::Result<ServerIdentity> {
    let c = core.clone();
    if let Some((cert, key)) = tokio::task::spawn_blocking(move || c.lan_identity()).await?? {
        return Ok(ServerIdentity::from_der(cert, key));
    }
    let identity = ServerIdentity::generate(&core.lan_server_name())?;
    let c = core.clone();
    let (cert, key) = (identity.cert_der().to_vec(), identity.key_der().to_vec());
    tokio::task::spawn_blocking(move || c.lan_store_identity(&cert, &key)).await??;
    Ok(identity)
}

/// Runs forever; spawn it once at start-up on the shell's async runtime.
pub async fn supervise(core: Arc<Core>, opts: ServerOptions) {
    let mut running: Option<LanServer> = None;
    loop {
        let c = core.clone();
        let want = tokio::task::spawn_blocking(move || c.lan_enabled()).await.unwrap_or(false);
        match (want, running.is_some()) {
            (true, false) => {
                let started = async {
                    let identity = load_or_create_identity(&core).await?;
                    LanServer::start(core.clone(), identity, opts.clone()).await
                }
                .await;
                match started {
                    Ok(server) => running = Some(server),
                    Err(e) => {
                        tracing::error!(error = %e, "LAN server could not start");
                        core.lan().set_runtime(Runtime::default());
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
            (false, true) => {
                if let Some(server) = running.take() {
                    server.stop(&core);
                    tracing::info!("LAN server stopped");
                }
            }
            _ => {}
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
