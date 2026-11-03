//! mDNS/DNS-SD: the server advertises `_artaveo._tcp.local.`; clients list
//! servers by clinic name — users never type an IP address.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::SERVICE_TYPE;

#[derive(Debug, Clone)]
pub struct Found {
    pub clinic_name: String,
    pub addr: String,
    /// First 8 hex chars of the cert fingerprint, shown next to the clinic
    /// name so two servers with the same name can be told apart.
    pub fp_hint: String,
}

/// Keep the returned daemon alive for as long as the server runs.
pub fn advertise(
    clinic_name: &str,
    instance: &str,
    port: u16,
    fp: &[u8; 32],
) -> anyhow::Result<ServiceDaemon> {
    let mdns = ServiceDaemon::new()?;
    let host = format!("{instance}.local.");
    let props: HashMap<String, String> = [
        ("clinic".to_string(), clinic_name.to_string()),
        ("fp".to_string(), hex::encode(&fp[..4])),
        ("v".to_string(), "1".to_string()),
    ]
    .into();
    let info = ServiceInfo::new(SERVICE_TYPE, instance, &host, "", port, props)?.enable_addr_auto();
    mdns.register(info)?;
    Ok(mdns)
}

pub fn browse(timeout: Duration) -> anyhow::Result<Vec<Found>> {
    let mdns = ServiceDaemon::new()?;
    let rx = mdns.browse(SERVICE_TYPE)?;
    let deadline = Instant::now() + timeout;
    let mut out: Vec<Found> = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                // Prefer IPv4: some consumer routers mangle IPv6 multicast.
                let Some(ip) = info
                    .get_addresses()
                    .iter()
                    .find(|a| a.is_ipv4())
                    .or(info.get_addresses().iter().next())
                    .copied()
                else {
                    continue;
                };
                out.push(Found {
                    clinic_name: info.get_property_val_str("clinic").unwrap_or_default().to_string(),
                    addr: format!("{ip}:{}", info.get_port()),
                    fp_hint: info.get_property_val_str("fp").unwrap_or_default().to_string(),
                });
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = mdns.shutdown();
    Ok(out)
}
