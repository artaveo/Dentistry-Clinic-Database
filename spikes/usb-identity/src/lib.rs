//! USB backup-device identity (Phase 10.3 "Trusted USB").
//!
//! Spike findings encoded here:
//! * Drive letters change between plug-ins → never used for identity.
//! * Volume serial (from `GetVolumeInformationW`) changes on every format.
//! * Hardware serial (`IOCTL_STORAGE_QUERY_PROPERTY`) is the best OS-level
//!   signal, but cheap sticks often report none, or a shared fake serial
//!   such as `0123456789ABCDEF`/`AA00000000011234`.
//! * External USB hard drives report `DRIVE_FIXED`, not `DRIVE_REMOVABLE`,
//!   so devices are filtered by **bus type = USB**, not by drive type.
//!
//! Decision: identity = signed **marker file** written on the device when the
//! Owner trusts it, cross-checked against hardware serial + vendor/product
//! when the device provides a credible serial.

pub mod marker;
#[cfg(windows)]
pub mod windows;

pub use marker::{Marker, MarkerError};

use serde::{Deserialize, Serialize};

/// What the OS tells us about a mounted USB volume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsbVolume {
    /// Current mount point, e.g. `E:\`. Informational only.
    pub mount: String,
    pub vendor: String,
    pub product: String,
    pub hardware_serial: Option<String>,
    pub volume_serial: u32,
    pub volume_label: String,
    pub filesystem: String,
}

/// Serials seen on many unrelated cheap controllers; treated as absent.
const BOGUS_SERIALS: &[&str] = &[
    "0123456789ABCDEF",
    "AA00000000011234",
    "0000000000000000",
    "00000000000000000000",
    "123456789ABC",
    "FFFFFFFFFFFFFFFF",
];

/// Normalises a serial and discards values that cannot identify a device.
pub fn credible_serial(raw: &str) -> Option<String> {
    let s: String = raw.trim().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let s = s.to_ascii_uppercase();
    if s.len() < 6 || BOGUS_SERIALS.contains(&s.as_str()) {
        return None;
    }
    if s.chars().all(|c| c == s.chars().next().unwrap()) {
        return None;
    }
    Some(s)
}

/// What we persist in the clinic DB when the Owner clicks "Use for Backup".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedDevice {
    pub device_id: String,
    pub vendor: String,
    pub product: String,
    pub hardware_serial: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Marker valid and hardware matches (or device has no credible serial).
    Trusted,
    /// Marker valid but hardware serial differs → marker was copied to another stick.
    Cloned,
    /// No valid marker for this clinic: ask "New Backup Device Detected".
    Unknown,
}

pub fn verify(vol: &UsbVolume, marker: Option<&Marker>, trusted: &[TrustedDevice]) -> Verdict {
    let Some(m) = marker else { return Verdict::Unknown };
    let Some(t) = trusted.iter().find(|t| t.device_id == m.device_id) else {
        return Verdict::Unknown;
    };
    match (&t.hardware_serial, vol.hardware_serial.as_deref().and_then(credible_serial)) {
        (Some(expected), Some(actual)) if *expected != actual => Verdict::Cloned,
        (Some(_), None) => Verdict::Cloned,
        _ => Verdict::Trusted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vol(serial: Option<&str>) -> UsbVolume {
        UsbVolume {
            mount: "E:\\".into(),
            vendor: "SanDisk".into(),
            product: "Ultra".into(),
            hardware_serial: serial.map(Into::into),
            volume_serial: 0x1234_ABCD,
            volume_label: "BACKUP".into(),
            filesystem: "exFAT".into(),
        }
    }

    #[test]
    fn bogus_serials_are_ignored() {
        assert_eq!(credible_serial(" 0123456789abcdef "), None);
        assert_eq!(credible_serial("AAAAAAAA"), None);
        assert_eq!(credible_serial("4C530001230321117283"), Some("4C530001230321117283".into()));
    }

    #[test]
    fn verdicts() {
        let m = Marker { device_id: "dev-1".into(), clinic_id: "c".into(), mac: String::new() };
        let trusted = vec![TrustedDevice {
            device_id: "dev-1".into(),
            vendor: "SanDisk".into(),
            product: "Ultra".into(),
            hardware_serial: Some("4C530001230321117283".into()),
        }];
        assert_eq!(verify(&vol(Some("4C530001230321117283")), Some(&m), &trusted), Verdict::Trusted);
        assert_eq!(verify(&vol(Some("4C530009999999999999")), Some(&m), &trusted), Verdict::Cloned);
        assert_eq!(verify(&vol(None), None, &trusted), Verdict::Unknown);

        let no_serial = vec![TrustedDevice { hardware_serial: None, ..trusted[0].clone() }];
        assert_eq!(verify(&vol(Some("0123456789ABCDEF")), Some(&m), &no_serial), Verdict::Trusted);
    }
}
