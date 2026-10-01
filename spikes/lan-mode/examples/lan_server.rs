//! Run on the "clinic server" PC:  cargo run -p lan-mode --example lan_server
use lan_mode::{discovery, server::Server, tls::ServerIdentity};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let name = "Artaveo Demo Clinic";
    let server = Server::new(name, ServerIdentity::generate(name)?);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:47800").await?;
    let _mdns = discovery::advertise(name, "artaveo-demo", 47800, &server.fingerprint())?;
    println!("Server fingerprint: {}", hex::encode(server.fingerprint()));
    println!("Pairing code (show on screen): {}", server.open_pairing());
    let s = server.clone();
    tokio::spawn(async move {
        let mut i = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            i += 1;
            s.publish("demo.tick", serde_json::json!({ "n": i }));
        }
    });
    server.serve(listener).await
}
