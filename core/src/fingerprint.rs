//! Hardware fingerprint of this computer (ADR-25). The vendor approves a computer by this value, so a
//! stolen license key is useless on any other machine.
//!
//! `SHA-256("artaveo-dental/fp/v1" NUL machine id)` in hex. The machine id is the Windows
//! `MachineGuid` (stable across reboots and updates; changes when Windows is reinstalled, which is
//! exactly when the vendor should look again). The MAC address is not used: it can be changed in
//! seconds and differs between Wi-Fi, USB adapters and VPNs.

use sha2::{Digest, Sha256};

const SALT: &[u8] = b"artaveo-dental/fp/v1\0";

/// Turns a raw machine id into the fingerprint (64 hex characters).
pub fn from_machine_id(id: &str) -> String {
    let mut h = Sha256::new();
    h.update(SALT);
    h.update(id.trim().to_ascii_lowercase().as_bytes());
    hex::encode(h.finalize())
}

/// "AB12-CD34": the first 8 characters, what a person compares on the phone or in the panel.
pub fn short(fingerprint: &str) -> String {
    let up = fingerprint.to_ascii_uppercase();
    let head: String = up.chars().take(8).collect();
    if head.len() == 8 {
        format!("{}-{}", &head[..4], &head[4..])
    } else {
        head
    }
}

#[cfg(windows)]
fn machine_id() -> Option<String> {
    use std::ptr;
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    // winreg.h value; windows-sys does not export it. Read the 64-bit view even from a 32-bit build.
    const RRF_SUBKEY_WOW64_64KEY: u32 = 0x0001_0000;
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let sub = wide("SOFTWARE\\Microsoft\\Cryptography");
    let name = wide("MachineGuid");
    let mut buf = [0u16; 128];
    let mut len = (buf.len() * 2) as u32;
    // SAFETY: both strings are NUL-terminated and live for the call; `buf` is `len` bytes long.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            sub.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW64_64KEY,
            ptr::null_mut(),
            buf.as_mut_ptr() as *mut _,
            &mut len,
        )
    };
    if status != 0 {
        return None;
    }
    let chars = (len as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..chars]))
}

#[cfg(not(windows))]
fn machine_id() -> Option<String> {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// This computer's fingerprint. When the system gives no machine id (very rare) a fixed placeholder
/// is hashed: the vendor then sees the same fingerprint for such machines and decides by hand.
pub fn current() -> String {
    from_machine_id(&machine_id().unwrap_or_else(|| "unknown-machine".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_a_stable_salted_hash() {
        let a = from_machine_id("ABCD-1234");
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_eq!(a, from_machine_id("  abcd-1234 "), "case and spaces do not matter");
        assert_ne!(a, from_machine_id("abcd-1235"));
        assert_ne!(a, hex::encode(Sha256::digest(b"abcd-1234")), "the salt is part of it");
        assert_eq!(current(), current());
    }

    #[test]
    fn the_short_form_is_what_a_person_reads_out() {
        assert_eq!(short(&from_machine_id("x")).len(), 9);
        assert_eq!(short("ab12cd34ef56"), "AB12-CD34");
        assert_eq!(short("abc"), "ABC");
    }
}
