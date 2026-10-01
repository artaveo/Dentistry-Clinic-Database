use super::{DataKey, KeyError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectorKind {
    /// Windows DPAPI, `CRYPTPROTECT_LOCAL_MACHINE` scope.
    WindowsDpapiMachine,
    /// Development-only fallback for non-Windows builds (CI on Linux).
    /// The key is stored *unprotected*; never shipped.
    InsecureDevFile,
}

impl ProtectorKind {
    pub fn code(self) -> &'static str {
        match self {
            ProtectorKind::WindowsDpapiMachine => "windows_dpapi_machine",
            ProtectorKind::InsecureDevFile => "insecure_dev_file",
        }
    }
}

/// Wraps the data key for at-rest storage on *this* computer.
pub trait KeyProtector: Send + Sync {
    fn kind(&self) -> ProtectorKind;
    fn protect(&self, key: &DataKey) -> Result<Vec<u8>, KeyError>;
    fn unprotect(&self, blob: &[u8]) -> Result<DataKey, KeyError>;
}

pub fn default_protector() -> Box<dyn KeyProtector> {
    #[cfg(windows)]
    {
        Box::new(dpapi::Dpapi)
    }
    #[cfg(not(windows))]
    {
        Box::new(DevFile)
    }
}

#[cfg(not(windows))]
struct DevFile;

#[cfg(not(windows))]
impl KeyProtector for DevFile {
    fn kind(&self) -> ProtectorKind {
        ProtectorKind::InsecureDevFile
    }
    fn protect(&self, key: &DataKey) -> Result<Vec<u8>, KeyError> {
        let mut v = b"DEV1".to_vec();
        v.extend_from_slice(key.as_bytes());
        Ok(v)
    }
    fn unprotect(&self, blob: &[u8]) -> Result<DataKey, KeyError> {
        if blob.len() != 36 || &blob[..4] != b"DEV1" {
            return Err(KeyError::Unwrap);
        }
        let mut k = [0u8; 32];
        k.copy_from_slice(&blob[4..]);
        Ok(DataKey::from_bytes(k))
    }
}

#[cfg(windows)]
mod dpapi {
    use super::*;
    use std::ptr;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_LOCAL_MACHINE, CRYPTPROTECT_UI_FORBIDDEN,
        CRYPT_INTEGER_BLOB,
    };

    /// Application entropy: a blob protected by another app on the same
    /// machine cannot be confused with ours (not a secret).
    const ENTROPY: &[u8] = b"Artaveo Dental/db-key/v1";

    pub struct Dpapi;

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
    }

    unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as _);
        v
    }

    impl KeyProtector for Dpapi {
        fn kind(&self) -> ProtectorKind {
            ProtectorKind::WindowsDpapiMachine
        }

        fn protect(&self, key: &DataKey) -> Result<Vec<u8>, KeyError> {
            let input = blob(key.as_bytes());
            let entropy = blob(ENTROPY);
            let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: ptr::null_mut() };
            // Machine scope: the LAN server may run as a Windows service and
            // clinic staff share PCs under different Windows accounts.
            let ok = unsafe {
                CryptProtectData(
                    &input,
                    ptr::null(),
                    &entropy,
                    ptr::null(),
                    ptr::null(),
                    CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out,
                )
            };
            if ok == 0 {
                return Err(KeyError::Os(std::io::Error::last_os_error().to_string()));
            }
            Ok(unsafe { take(out) })
        }

        fn unprotect(&self, data: &[u8]) -> Result<DataKey, KeyError> {
            let input = blob(data);
            let entropy = blob(ENTROPY);
            let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: ptr::null_mut() };
            let ok = unsafe {
                CryptUnprotectData(
                    &input,
                    ptr::null_mut(),
                    &entropy,
                    ptr::null(),
                    ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out,
                )
            };
            if ok == 0 {
                return Err(KeyError::Os(std::io::Error::last_os_error().to_string()));
            }
            let mut v = unsafe { take(out) };
            if v.len() != 32 {
                return Err(KeyError::Unwrap);
            }
            let mut k = [0u8; 32];
            k.copy_from_slice(&v);
            zeroize::Zeroize::zeroize(&mut v);
            Ok(DataKey::from_bytes(k))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protect_roundtrip() {
        let p = default_protector();
        let k = DataKey::generate();
        let blob = p.protect(&k).unwrap();
        assert_ne!(blob.as_slice(), k.as_bytes().as_slice()[..].as_ref() as &[u8]);
        assert_eq!(p.unprotect(&blob).unwrap(), k);
    }
}
