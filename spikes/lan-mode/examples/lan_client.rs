//! Run on a reception/doctor PC:  cargo run -p lan-mode --example lan_client -- <code>
use std::time::{Duration, Instant};

use lan_mode::{client, discovery};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let code = std::env::args().nth(1).expect("pairing code shown on the server");
    let found = tokio::task::spawn_blocking(|| discovery::browse(Duration::from_secs(3))).await??;
    let srv = found.first().ok_or_else(|| anyhow::anyhow!("no clinic server found on this network"))?;
    println!("found {} at {} (fp {})", srv.clinic_name, srv.addr, srv.fp_hint);
    let paired = client::pair(&srv.addr, &code, "Reception PC").await?;
    let (conn, mut events) = client::connect(&srv.addr, &paired).await?;
    let t = Instant::now();
    for _ in 0..100 {
        conn.call("ping", serde_json::json!({})).await?;
    }
    println!("connected to {}; avg RPC {:?}", conn.server_name, t.elapsed() / 100);
    while let Some(ev) = events.recv().await {
        println!("event: {ev:?}");
    }
    Ok(())
}
