//! Trusted USB backup devices (roadmap 10.3, ADR-22, Spike 8).
//!
//! A drive letter changes between plug-ins and a volume serial changes on every format, so neither
//! is an identity. A device becomes *trusted* when the Owner clicks "Use for Backup": a signed
//! marker file (`ArtaveoBackup/device.json`, HMAC with a key derived from the clinic's data key) is
//! written on it and its hardware serial (when it has a credible one) is stored in the database.
//! Later: marker valid + serial matches → [`Verdict::Trusted`] and backups go there on their own.
//! Marker valid but another serial → [`Verdict::Cloned`] (the marker was copied to a different
//! stick): nothing is written there and the Owner is warned.

use std::path::{Path, PathBuf};

use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::Result;
use crate::keys::DataKey;

pub const MARKER_DIR: &str = "ArtaveoBackup";
pub const MARKER_FILE: &str = "device.json";

/// What the operating system tells about one mounted USB volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbVolume {
    /// Current mount point such as `E:\`. Shown to the user; never used as an identity.
    pub mount: PathBuf,
    pub vendor: String,
    pub product: String,
    pub hardware_serial: Option<String>,
    pub label: String,
    pub filesystem: String,
}

/// Where the list of connected devices comes from; tests plug in a fake one.
pub trait UsbProvider: Send + Sync {
    fn volumes(&self) -> Vec<UsbVolume>;
}

/// The real thing: Windows asks the storage driver which volumes sit on the USB bus. Other systems
/// (the Linux CI machine) report none.
pub struct SystemUsb;

impl UsbProvider for SystemUsb {
    fn volumes(&self) -> Vec<UsbVolume> {
        #[cfg(windows)]
        {
            windows::usb_volumes()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }
}

/// Free bytes on the volume that holds `path`, when the system can tell.
pub fn free_bytes(path: &Path) -> Option<i64> {
    #[cfg(windows)]
    {
        windows::free_bytes(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

/// Serials seen on many unrelated cheap controllers; they identify nothing.
const BOGUS_SERIALS: &[&str] =
    &["0123456789ABCDEF", "AA00000000011234", "0000000000000000", "00000000000000000000", "123456789ABC", "FFFFFFFFFFFFFFFF"];

/// Normalises a serial and drops values that cannot tell two devices apart.
pub fn credible_serial(raw: &str) -> Option<String> {
    let s: String = raw.trim().chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase();
    if s.len() < 6 || BOGUS_SERIALS.contains(&s.as_str()) {
        return None;
    }
    let first = s.chars().next()?;
    if s.chars().all(|c| c == first) {
        return None;
    }
    Some(s)
}

/// A device the Owner trusted (a row of `backup_device`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trusted {
    pub device_id: String,
    pub hardware_serial: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Trusted,
    Cloned,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub device_id: String,
    pub clinic_id: String,
    pub mac: String,
}

fn mac_key(data_key: &DataKey) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, data_key.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(b"artaveo-dental/usb-marker/v1", &mut okm).expect("32 bytes is a valid HKDF length");
    okm
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

/// Folder on the device that holds the backups.
pub fn backup_folder(mount: &Path) -> PathBuf {
    mount.join(MARKER_DIR)
}

impl Marker {
    pub fn issue(clinic_id: &str, data_key: &DataKey) -> Self {
        let mut id = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut id);
        let device_id = hex::encode(id);
        let mac = hex::encode(mac_state(&mac_key(data_key), &device_id, clinic_id).finalize().into_bytes());
        Self { device_id, clinic_id: clinic_id.into(), mac }
    }

    pub fn write(&self, mount: &Path) -> Result<()> {
        let p = marker_path(mount);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&p, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    /// The marker of this clinic on the device, or `None` when there is none, it is for another
    /// clinic, or its signature is wrong (a forged or damaged file counts as "not trusted").
    pub fn read_verified(mount: &Path, clinic_id: &str, data_key: &DataKey) -> Option<Self> {
        let raw = std::fs::read(marker_path(mount)).ok()?;
        let m: Marker = serde_json::from_slice(&raw).ok()?;
        if m.clinic_id != clinic_id {
            return None;
        }
        let given = hex::decode(&m.mac).ok()?;
        mac_state(&mac_key(data_key), &m.device_id, &m.clinic_id).verify_slice(&given).ok()?;
        Some(m)
    }
}

pub fn verify(vol: &UsbVolume, marker: Option<&Marker>, trusted: &[Trusted]) -> Verdict {
    let Some(m) = marker else { return Verdict::Unknown };
    let Some(t) = trusted.iter().find(|t| t.device_id == m.device_id) else { return Verdict::Unknown };
    match (&t.hardware_serial, vol.hardware_serial.as_deref().and_then(credible_serial)) {
        (Some(expected), Some(actual)) if *expected != actual => Verdict::Cloned,
        (Some(_), None) => Verdict::Cloned,
        _ => Verdict::Trusted,
    }
}

#[cfg(windows)]
mod windows {
    //! Windows enumeration of USB volumes. No administrator rights: the volume handle is opened with
    //! zero access rights, which is enough for `IOCTL_STORAGE_QUERY_PROPERTY`.

    use std::path::{Path, PathBuf};
    use std::ptr;

    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        BusTypeUsb, CreateFileW, GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::{
        PropertyStandardQuery, StorageDeviceProperty, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_DEVICE_DESCRIPTOR,
        STORAGE_PROPERTY_QUERY,
    };
    use windows_sys::Win32::System::WindowsProgramming::{DRIVE_FIXED, DRIVE_REMOVABLE};
    use windows_sys::Win32::System::IO::DeviceIoControl;

    use super::{credible_serial, UsbVolume};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide(buf: &[u16]) -> String {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..len])
    }

    struct Descriptor {
        bus_type: i32,
        vendor: String,
        product: String,
        serial: Option<String>,
    }

    fn ascii_at(buf: &[u8], off: u32) -> String {
        if off == 0 || off as usize >= buf.len() {
            return String::new();
        }
        let s = &buf[off as usize..];
        let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
        String::from_utf8_lossy(&s[..end]).trim().to_string()
    }

    fn query_descriptor(letter: char) -> Option<Descriptor> {
        let path = wide(&format!("\\\\.\\{letter}:"));
        // SAFETY: plain Win32 calls with buffers that live for the whole call; the handle is closed.
        unsafe {
            let h = CreateFileW(
                path.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            );
            if h == INVALID_HANDLE_VALUE {
                return None;
            }
            let query = STORAGE_PROPERTY_QUERY {
                PropertyId: StorageDeviceProperty,
                QueryType: PropertyStandardQuery,
                AdditionalParameters: [0],
            };
            // u64 elements keep the buffer aligned for the descriptor struct.
            let mut buf = vec![0u64; 128];
            let mut returned = 0u32;
            let ok = DeviceIoControl(
                h,
                IOCTL_STORAGE_QUERY_PROPERTY,
                &query as *const _ as *const _,
                std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                buf.as_mut_ptr() as *mut _,
                (buf.len() * 8) as u32,
                &mut returned,
                ptr::null_mut(),
            );
            CloseHandle(h);
            if ok == 0 || (returned as usize) < std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
                return None;
            }
            let bytes = std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len() * 8);
            let d = &*(buf.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR);
            Some(Descriptor {
                bus_type: d.BusType,
                vendor: ascii_at(bytes, d.VendorIdOffset),
                product: ascii_at(bytes, d.ProductIdOffset),
                serial: credible_serial(&ascii_at(bytes, d.SerialNumberOffset)),
            })
        }
    }

    fn volume_info(root: &str) -> Option<(String, String)> {
        let root_w = wide(root);
        let mut label = [0u16; 261];
        let mut fs = [0u16; 261];
        // SAFETY: both buffers are valid for the lengths passed.
        let ok = unsafe {
            GetVolumeInformationW(
                root_w.as_ptr(),
                label.as_mut_ptr(),
                label.len() as u32,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                fs.as_mut_ptr(),
                fs.len() as u32,
            )
        };
        (ok != 0).then(|| (from_wide(&label), from_wide(&fs)))
    }

    /// Every mounted volume whose device sits on the USB bus: flash sticks (`DRIVE_REMOVABLE`) and
    /// external disks (`DRIVE_FIXED`).
    pub fn usb_volumes() -> Vec<UsbVolume> {
        // SAFETY: no arguments.
        let mask = unsafe { GetLogicalDrives() };
        let mut out = Vec::new();
        for i in 0..26u32 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let letter = (b'A' + i as u8) as char;
            let root = format!("{letter}:\\");
            // SAFETY: NUL-terminated wide string that outlives the call.
            let dt = unsafe { GetDriveTypeW(wide(&root).as_ptr()) };
            if dt != DRIVE_REMOVABLE && dt != DRIVE_FIXED {
                continue;
            }
            let Some(desc) = query_descriptor(letter) else { continue };
            if desc.bus_type != BusTypeUsb {
                continue;
            }
            let Some((label, filesystem)) = volume_info(&root) else { continue };
            out.push(UsbVolume {
                mount: PathBuf::from(root),
                vendor: desc.vendor,
                product: desc.product,
                hardware_serial: desc.serial,
                label,
                filesystem,
            });
        }
        out
    }

    pub fn free_bytes(path: &Path) -> Option<i64> {
        let w = wide(&path.to_string_lossy());
        let mut free = 0u64;
        // SAFETY: the output pointer is valid; the other two are optional.
        let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut free, ptr::null_mut(), ptr::null_mut()) };
        (ok != 0).then_some(free as i64)
    }
}

#[cfg(test)]
pub use fake::FakeUsb;

#[cfg(test)]
mod fake {
    use std::sync::Mutex;

    use super::{UsbProvider, UsbVolume};

    /// Tests plug and unplug devices by editing the list.
    #[derive(Default)]
    pub struct FakeUsb(pub Mutex<Vec<UsbVolume>>);

    impl UsbProvider for FakeUsb {
        fn volumes(&self) -> Vec<UsbVolume> {
            self.0.lock().unwrap().clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vol(serial: Option<&str>) -> UsbVolume {
        UsbVolume {
            mount: PathBuf::from("E:\\"),
            vendor: "SanDisk".into(),
            product: "Ultra".into(),
            hardware_serial: serial.map(Into::into),
            label: "BACKUP".into(),
            filesystem: "exFAT".into(),
        }
    }

    #[test]
    fn bogus_serials_are_ignored() {
        assert_eq!(credible_serial(" 0123456789abcdef "), None);
        assert_eq!(credible_serial("AAAAAAAA"), None);
        assert_eq!(credible_serial("abc"), None);
        assert_eq!(credible_serial("4C530001230321117283"), Some("4C530001230321117283".into()));
    }

    #[test]
    fn marker_is_signed_per_clinic_and_forgery_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let key = DataKey::generate();
        let m = Marker::issue("clinic-1", &key);
        m.write(dir.path()).unwrap();
        assert_eq!(Marker::read_verified(dir.path(), "clinic-1", &key), Some(m.clone()));
        assert_eq!(Marker::read_verified(dir.path(), "clinic-2", &key), None, "another clinic");
        assert_eq!(Marker::read_verified(dir.path(), "clinic-1", &DataKey::generate()), None, "another key");
        Marker { device_id: "attacker".into(), ..m }.write(dir.path()).unwrap();
        assert_eq!(Marker::read_verified(dir.path(), "clinic-1", &key), None, "forged marker");
    }

    #[test]
    fn verdicts() {
        let m = Marker { device_id: "dev-1".into(), clinic_id: "c".into(), mac: String::new() };
        let trusted = vec![Trusted { device_id: "dev-1".into(), hardware_serial: Some("4C530001230321117283".into()) }];
        assert_eq!(verify(&vol(Some("4C530001230321117283")), Some(&m), &trusted), Verdict::Trusted);
        assert_eq!(verify(&vol(Some("4C530009999999999999")), Some(&m), &trusted), Verdict::Cloned);
        assert_eq!(verify(&vol(None), Some(&m), &trusted), Verdict::Cloned);
        assert_eq!(verify(&vol(None), None, &trusted), Verdict::Unknown);
        let other = Marker { device_id: "dev-9".into(), ..m.clone() };
        assert_eq!(verify(&vol(None), Some(&other), &trusted), Verdict::Unknown);
        let no_serial = vec![Trusted { device_id: "dev-1".into(), hardware_serial: None }];
        assert_eq!(verify(&vol(Some("0123456789ABCDEF")), Some(&m), &no_serial), Verdict::Trusted);
    }
}
