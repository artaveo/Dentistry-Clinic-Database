//! Phase 7 exit criteria in code: one server and three clients run a working day over real
//! sockets (TLS + WebSocket, loopback) against the real Core, without any manual network setup.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use artaveo_core::{Config, Core, Environment};
use artaveo_lan::client::{self, ClientError, LanClient, Notice};
use artaveo_lan::server::{LanServer, ServerOptions};
use artaveo_lan::service;
use artaveo_shared::{methods as m, ErrorCode, RpcRequest, RpcResponse};
use serde_json::{json, Value};
use tempfile::TempDir;

struct Clinic {
    _dir: TempDir,
    core: Arc<Core>,
    server: LanServer,
    addr: String,
    owner: String,
}

fn local(core: &Core, method: &str, params: Value, token: Option<&str>) -> Value {
    match core.handle(RpcRequest { method: method.into(), params, token: token.map(Into::into) }) {
        RpcResponse::Ok { result } => result,
        RpcResponse::Error { error } => panic!("{method} failed: {:?}: {}", error.code, error.detail),
    }
}

async fn clinic() -> Clinic {
    let dir = tempfile::tempdir().unwrap();
    let core = Arc::new(Core::open(Config::new(Environment::Test, dir.path().to_path_buf())).unwrap());
    local(
        &core,
        m::APP_SETUP,
        json!({
            "clinic_name": "Kabul Smile", "owner_username": "owner", "owner_display_name": "Owner",
            "owner_password": "owner-pass-1", "language": "en"
        }),
        None,
    );
    let owner = local(&core, m::AUTH_LOGIN, json!({"username": "owner", "password": "owner-pass-1"}), None)
        ["token"]
        .as_str()
        .unwrap()
        .to_string();
    local(&core, m::LAN_ENABLE, json!({}), Some(&owner));
    let identity = service::load_or_create_identity(&core).await.unwrap();
    let opts = ServerOptions { bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST), ports: vec![0], advertise: false };
    let server = LanServer::start(core.clone(), identity, opts).await.unwrap();
    let addr = format!("127.0.0.1:{}", server.port);
    Clinic { _dir: dir, core, server, addr, owner }
}

impl Clinic {
    /// Owner clicks "Add computer", the new computer types the code.
    async fn add_computer(&self, name: &str) -> Arc<LanClient> {
        let code = local(&self.core, m::LAN_PAIRING_OPEN, json!({}), Some(&self.owner))["code"]
            .as_str()
            .unwrap()
            .to_string();
        let paired = client::pair(&self.addr, &code, name).await.unwrap();
        let c = LanClient::new(paired, None);
        c.spawn();
        wait_online(&c).await;
        c
    }

    fn user(&self, username: &str, role: &str) {
        local(
            &self.core,
            m::USERS_CREATE,
            json!({"username": username, "display_name": username, "password": "staff-pass-12", "role": role}),
            Some(&self.owner),
        );
    }
}

async fn wait_online(c: &LanClient) {
    for _ in 0..100 {
        if c.is_online() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("client did not connect");
}

async fn rpc(c: &LanClient, method: &str, params: Value, token: Option<&str>) -> Result<Value, ErrorCode> {
    match c.call(RpcRequest { method: method.into(), params, token: token.map(Into::into) }).await {
        RpcResponse::Ok { result } => Ok(result),
        RpcResponse::Error { error } => Err(error.code),
    }
}

async fn login(c: &LanClient, user: &str) -> String {
    rpc(c, m::AUTH_LOGIN, json!({"username": user, "password": "staff-pass-12"}), None).await.unwrap()
        ["token"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_server_and_three_clients_run_a_working_day() {
    let clinic = clinic().await;
    clinic.user("reception", "receptionist");
    clinic.user("admin1", "administrator");
    clinic.user("admin2", "administrator");

    let reception = clinic.add_computer("Reception").await;
    let doctor1 = clinic.add_computer("Doctor 1").await;
    let doctor2 = clinic.add_computer("Doctor 2").await;
    let (rt, d1t, d2t) = (
        login(&reception, "reception").await,
        login(&doctor1, "admin1").await,
        login(&doctor2, "admin2").await,
    );

    // Real-time: the patient the reception registers is announced to the other screens.
    let mut seen_by_doctor = doctor1.subscribe();
    let p = rpc(
        &reception,
        m::PATIENTS_CREATE,
        json!({"full_name": "احمد خان", "phone": "0700123456", "allow_duplicate": true}),
        Some(&rt),
    )
    .await
    .unwrap();
    let event = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(Notice::Live(e)) = seen_by_doctor.recv().await {
                if e.topic == "patients" {
                    return e;
                }
            }
        }
    })
    .await
    .expect("the doctor's screen hears about the new patient");
    assert_eq!(event.method, "patients.create");
    let listed = rpc(&doctor2, m::PATIENTS_LIST, json!({}), Some(&d2t)).await.unwrap();
    assert_eq!(listed["total"], 1);

    // Optimistic locking: two computers edit the same record; the second save is refused.
    let mut a = p.clone();
    a["status"] = json!("active");
    a["full_name"] = json!("احمد خان (الف)");
    let mut b = a.clone();
    b["full_name"] = json!("احمد خان (ب)");
    rpc(&doctor1, m::PATIENTS_UPDATE, a, Some(&d1t)).await.unwrap();
    assert_eq!(rpc(&doctor2, m::PATIENTS_UPDATE, b, Some(&d2t)).await, Err(ErrorCode::Conflict));

    // "Who has this record open".
    let rec = json!({"entity": "patient", "id": p["id"]});
    assert!(rpc(&doctor1, m::PRESENCE_ENTER, rec.clone(), Some(&d1t))
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    let others = rpc(&doctor2, m::PRESENCE_ENTER, rec.clone(), Some(&d2t)).await.unwrap();
    assert_eq!(others.as_array().unwrap().len(), 1);
    assert_eq!(others[0]["user_name"], "admin1");
    assert_eq!(others[0]["device"], "Doctor 1");
    rpc(&doctor1, m::PRESENCE_LEAVE, rec.clone(), Some(&d1t)).await.unwrap();
    assert!(rpc(&doctor2, m::PRESENCE_WHO, rec, Some(&d2t)).await.unwrap().as_array().unwrap().is_empty());

    // A busy morning: all three computers register patients at the same time, nothing is lost.
    let batch = |c: Arc<LanClient>, token: String, tag: &'static str| async move {
        for i in 0..15 {
            rpc(
                &c,
                m::PATIENTS_CREATE,
                json!({"full_name": format!("{tag} {i}"), "phone": format!("07{:08}", i), "allow_duplicate": true}),
                Some(&token),
            )
            .await
            .unwrap();
        }
    };
    tokio::join!(
        batch(reception.clone(), rt.clone(), "R"),
        batch(doctor1.clone(), d1t.clone(), "A"),
        batch(doctor2.clone(), d2t.clone(), "B"),
    );
    let all = rpc(&reception, m::PATIENTS_LIST, json!({"limit": 200}), Some(&rt)).await.unwrap();
    assert_eq!(all["total"], 46);
    let mut numbers: Vec<String> = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["patient_number"].as_str().unwrap().to_string())
        .collect();
    numbers.sort();
    numbers.dedup();
    assert_eq!(numbers.len(), 46, "every patient has its own number");

    // The server's list of computers.
    let devices = local(&clinic.core, m::LAN_DEVICES, json!({}), Some(&clinic.owner));
    assert_eq!(devices.as_array().unwrap().len(), 3);
    assert!(devices.as_array().unwrap().iter().all(|d| d["connected"] == true));
    let status = local(&clinic.core, m::LAN_STATUS, json!({}), Some(&clinic.owner));
    assert_eq!(status["connected_count"], 3);
    assert_eq!(status["running"], true);

    // Permissions are the server's: reception cannot manage computers, even over the network.
    assert_eq!(rpc(&reception, m::LAN_DEVICES, json!({}), Some(&rt)).await, Err(ErrorCode::Forbidden));

    // Latency target: ordinary calls under 300 ms (loopback gives the protocol's own floor).
    let t = Instant::now();
    for _ in 0..100 {
        rpc(&doctor1, m::SESSION_STATE, json!({}), Some(&d1t)).await.unwrap();
    }
    let avg = t.elapsed() / 100;
    assert!(avg < Duration::from_millis(100), "average round trip {avg:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_code_is_limited_and_adds_nothing() {
    let clinic = clinic().await;
    let code = local(&clinic.core, m::LAN_PAIRING_OPEN, json!({}), Some(&clinic.owner))["code"]
        .as_str()
        .unwrap()
        .to_string();
    let wrong = if code == "000000" { "000001" } else { "000000" };
    for left in (0..5).rev() {
        match client::pair(&clinic.addr, wrong, "Attacker").await {
            Err(ClientError::WrongCode { attempts_left }) => assert_eq!(attempts_left, left),
            other => panic!("{other:?}"),
        }
    }
    // After five failures the window is closed; even the right code no longer works.
    assert!(matches!(client::pair(&clinic.addr, &code, "Reception").await, Err(ClientError::Closed)));
    let devices = local(&clinic.core, m::LAN_DEVICES, json!({}), Some(&clinic.owner));
    assert!(devices.as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_revoked_computer_is_cut_off_and_cannot_come_back() {
    let clinic = clinic().await;
    clinic.user("reception", "receptionist");
    let pc = clinic.add_computer("Reception").await;
    let t = login(&pc, "reception").await;
    assert!(rpc(&pc, m::PATIENTS_LIST, json!({}), Some(&t)).await.is_ok());

    let id = local(&clinic.core, m::LAN_DEVICES, json!({}), Some(&clinic.owner))[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    local(&clinic.core, m::LAN_REVOKE, json!({"id": id}), Some(&clinic.owner));

    for _ in 0..50 {
        if !pc.is_online() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(!pc.is_online(), "the connection is closed");
    assert_eq!(rpc(&pc, m::PATIENTS_LIST, json!({}), Some(&t)).await, Err(ErrorCode::ServerUnreachable));
    let devices = local(&clinic.core, m::LAN_DEVICES, json!({}), Some(&clinic.owner));
    assert_eq!(devices[0]["revoked"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turning_server_mode_off_disconnects_everyone() {
    let clinic = clinic().await;
    let pc = clinic.add_computer("Reception").await;
    local(&clinic.core, m::LAN_DISABLE, json!({}), Some(&clinic.owner));
    for _ in 0..50 {
        if !pc.is_online() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(!pc.is_online());
    clinic.server.stop(&clinic.core);
    let status = local(&clinic.core, m::LAN_STATUS, json!({}), Some(&clinic.owner));
    assert_eq!(status["enabled"], false);
    assert_eq!(status["running"], false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_refuses_a_server_with_another_certificate() {
    let real = clinic().await;
    let paired = {
        let code = local(&real.core, m::LAN_PAIRING_OPEN, json!({}), Some(&real.owner))["code"]
            .as_str()
            .unwrap()
            .to_string();
        client::pair(&real.addr, &code, "Reception").await.unwrap()
    };
    // Another machine answers at the address the client remembers (DHCP, or an impostor).
    let impostor = clinic().await;
    let mut to_impostor = paired.clone();
    to_impostor.addr = impostor.addr.clone();
    let c = LanClient::new(to_impostor, None);
    c.spawn();
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(!c.is_online());
}
