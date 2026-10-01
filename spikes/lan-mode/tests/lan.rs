use std::time::{Duration, Instant};

use lan_mode::client::{self, ClientError};
use lan_mode::server::Server;
use lan_mode::tls::ServerIdentity;

async fn start(name: &str) -> (Server, String) {
    let server = Server::new(name, ServerIdentity::generate(name).unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(server.clone().serve(listener));
    (server, addr)
}

#[tokio::test]
async fn pair_connect_rpc_and_realtime_push() {
    let (server, addr) = start("Clinic A").await;
    let code = server.open_pairing();
    let reception = client::pair(&addr, &code, "Reception").await.unwrap();
    assert_eq!(reception.fingerprint, server.fingerprint());

    let code = server.open_pairing();
    let doctor = client::pair(&addr, &code, "Doctor 1").await.unwrap();
    assert_eq!(server.paired_devices().len(), 2);

    let (rc, _rev) = client::connect(&addr, &reception).await.unwrap();
    let (_dc, mut dev) = client::connect(&addr, &doctor).await.unwrap();
    assert_eq!(rc.server_name, "Clinic A");

    // Reception checks a patient in; the doctor's screen gets the push.
    rc.call("queue.check_in", serde_json::json!({"patient": "P-000123"})).await.unwrap();
    let ev = tokio::time::timeout(Duration::from_secs(2), dev.recv()).await.unwrap().unwrap();
    assert!(matches!(ev, lan_mode::protocol::Frame::Event { ref topic, .. } if topic == "queue.changed"));

    // NFR: LAN latency < 300 ms. Loopback gives the protocol overhead floor.
    let n = 500;
    let t = Instant::now();
    for i in 0..n {
        rc.call("ping", serde_json::json!({ "i": i })).await.unwrap();
    }
    let avg = t.elapsed() / n;
    eprintln!("avg RPC round-trip over TLS+WebSocket (loopback): {avg:?}");
    assert!(avg < Duration::from_millis(20));
}

#[tokio::test]
async fn wrong_code_is_limited_and_reveals_nothing() {
    let (server, addr) = start("Clinic A").await;
    let code = server.open_pairing();
    let wrong = if code == "000000" { "000001" } else { "000000" };
    for left in (1..5).rev() {
        match client::pair(&addr, wrong, "Attacker").await {
            Err(ClientError::WrongCode { attempts_left }) => assert_eq!(attempts_left, left),
            other => panic!("{other:?}"),
        }
    }
    assert!(matches!(
        client::pair(&addr, wrong, "Attacker").await,
        Err(ClientError::WrongCode { attempts_left: 0 })
    ));
    // Window is closed after 5 failures; even the right code no longer works.
    assert!(matches!(client::pair(&addr, &code, "Reception").await, Err(ClientError::Refused(_))));
    assert!(server.paired_devices().is_empty());
}

#[tokio::test]
async fn pinned_client_refuses_impostor_server() {
    let (server, addr) = start("Clinic A").await;
    let paired = client::pair(&addr, &server.open_pairing(), "Reception").await.unwrap();

    // A different machine answering on the same address (e.g. DHCP reassigned
    // the IP, or an attacker) has a different certificate.
    let (_impostor, impostor_addr) = start("Clinic A").await;
    assert!(client::connect(&impostor_addr, &paired).await.is_err());
}

#[tokio::test]
async fn unknown_device_token_is_rejected() {
    let (server, addr) = start("Clinic A").await;
    let mut paired = client::pair(&addr, &server.open_pairing(), "Reception").await.unwrap();
    paired.token = "00".repeat(32);
    assert!(matches!(client::connect(&addr, &paired).await, Err(ClientError::AuthFailed)));
}

/// Needs a network interface with multicast; disabled in sandboxed CI.
/// Run manually on a real LAN: `cargo test -p lan-mode -- --ignored`.
#[test]
#[ignore]
fn mdns_discovery_on_real_network() {
    let fp = [7u8; 32];
    let _d = lan_mode::discovery::advertise("Clinic mDNS", "artaveo-test", 47801, &fp).unwrap();
    let found = lan_mode::discovery::browse(Duration::from_secs(4)).unwrap();
    assert!(found.iter().any(|f| f.clinic_name == "Clinic mDNS" && f.fp_hint == "07070707"), "{found:?}");
}
