//! Marker file written to the root of a trusted backup device:
//! `ArtaveoBackup/device.json`. The MAC binds it to one clinic so a stick
//! trusted by clinic A is "unknown" to clinic B, and the file cannot be
//! forged without the clinic's secret (stored in the encrypted DB).

use std::path::{Path, PathBuf};

use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

pub const MARKER_DIR: &str = "ArtaveoBackup";
pub const MARKER_FILE: &str = "device.json";

#[derive(Debug, thiserror::Error)]
pub enum MarkerError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("marker signature is invalid")]
    BadMac,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub device_id: String,
    pub clinic_id: String,
    pub mac: String,
}

fn mac_state(secret: &[u8], device_id: &str, clinic_id: &str) -> Hmac<Sha256> {
    let mut m = Hmac::<Sha256>::new_from_slice(secret).expect("any key length");
    m.update(b"artaveo-usb-marker/v1\0");
    m.update(device_id.as_bytes());
    m.update(b"\0");
    m.update(clinic_id.as_bytes());
    m
}

pub fn marker_path(mount: &Path) -> PathBuf {
    mount.join(MARKER_DIR).join(MARKER_FILE)
}

impl Marker {
    pub fn issue(clinic_id: &str, secret: &[u8]) -> Self {
        let mut id = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut id);
        let device_id = hex::encode(id);
        let mac = hex::encode(mac_state(secret, &device_id, clinic_id).finalize().into_bytes());
        Self { device_id, clinic_id: clinic_id.into(), mac }
    }

    pub fn write(&self, mount: &Path) -> Result<(), MarkerError> {
        let p = marker_path(mount);
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(&p, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    /// `Ok(None)` when the device has no marker (never trusted).
    pub fn read_verified(mount: &Path, clinic_id: &str, secret: &[u8]) -> Result<Option<Self>, MarkerError> {
        let p = marker_path(mount);
        if !p.exists() {
            return Ok(None);
        }
        let m: Marker = serde_json::from_slice(&std::fs::read(p)?)?;
        if m.clinic_id != clinic_id {
            return Ok(None);
        }
        // Constant-time comparison via the Mac API.
        let given = hex::decode(&m.mac).map_err(|_| MarkerError::BadMac)?;
        mac_state(secret, &m.device_id, &m.clinic_id)
            .verify_slice(&given)
            .map_err(|_| MarkerError::BadMac)?;
        Ok(Some(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_write_verify_and_tamper() {
        let dir = tempfile::tempdir().unwrap();
        let secret = b"clinic-secret-from-encrypted-db";
        let m = Marker::issue("clinic-1", secret);
        m.write(dir.path()).unwrap();
        assert_eq!(Marker::read_verified(dir.path(), "clinic-1", secret).unwrap(), Some(m.clone()));
        // Another clinic sees an unknown device.
        assert_eq!(Marker::read_verified(dir.path(), "clinic-2", secret).unwrap(), None);
        // Forged marker.
        let forged = Marker { device_id: "attacker".into(), ..m };
        forged.write(dir.path()).unwrap();
        assert!(matches!(Marker::read_verified(dir.path(), "clinic-1", secret), Err(MarkerError::BadMac)));
    }
}
