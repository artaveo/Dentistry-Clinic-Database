//! Attachment file storage (roadmap 3.5, ADR-12): bytes live under the app
//! data folder, named by content hash (SHA-256) — same file written twice
//! (even for different patients) is stored once — and are encrypted with a
//! key derived from the database's own data key (same HKDF/XChaCha20-Poly1305
//! construction as the Recovery Key wrap, `keys::recovery`), so a stolen
//! laptop does not expose patient images any more than it exposes the database.

use std::fs;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::error::{CoreError, Result};
use crate::keys::DataKey;

const MAGIC: &[u8; 4] = b"AAT1";
const HKDF_INFO: &[u8] = b"artaveo-dental/attachment/v1";
/// 20 MiB: generous for dental X-rays, intra-oral photos and scanned documents.
pub const MAX_BYTES: usize = 20 * 1024 * 1024;

fn file_key(data_key: &DataKey) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, data_key.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(HKDF_INFO, &mut okm).expect("32 bytes is a valid HKDF length");
    okm
}

fn path_for(dir: &Path, sha256: &str) -> PathBuf {
    // Two-level fan-out so the folder never holds tens of thousands of files directly.
    dir.join(&sha256[0..2]).join(format!("{sha256}.bin"))
}

/// Encrypts and writes `bytes` if not already stored; returns the hex SHA-256
/// of the plaintext (the attachment's content address) and its size.
pub fn store(dir: &Path, data_key: &DataKey, bytes: &[u8]) -> Result<(String, i64)> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(CoreError::validation("attachment must be 1 byte - 20 MiB"));
    }
    let sha256 = hex::encode(Sha256::digest(bytes));
    let path = path_for(dir, &sha256);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut key = file_key(data_key);
        let mut nonce = [0u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new((&key).into());
        key.zeroize();
        let ct = cipher
            .encrypt(XNonce::from_slice(&nonce), bytes)
            .map_err(|_| CoreError::validation("attachment encryption failed"))?;
        let mut out = Vec::with_capacity(4 + 24 + ct.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        // Write-then-rename: a crash never leaves a half-written content file.
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, out)?;
        fs::rename(&tmp, &path)?;
    }
    Ok((sha256, bytes.len() as i64))
}

/// Reads and decrypts a previously stored attachment.
pub fn read(dir: &Path, data_key: &DataKey, sha256: &str) -> Result<Vec<u8>> {
    let raw = fs::read(path_for(dir, sha256))?;
    if raw.len() < 4 + 24 || &raw[..4] != MAGIC {
        return Err(CoreError::validation("attachment file is corrupt"));
    }
    let nonce = &raw[4..28];
    let mut key = file_key(data_key);
    let cipher = XChaCha20Poly1305::new((&key).into());
    key.zeroize();
    cipher
        .decrypt(XNonce::from_slice(nonce), &raw[28..])
        .map_err(|_| CoreError::validation("attachment is corrupt or the key is wrong"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_dedupes_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let key = DataKey::generate();
        let (hash1, size1) = store(dir.path(), &key, b"xray bytes").unwrap();
        let (hash2, _) = store(dir.path(), &key, b"xray bytes").unwrap();
        assert_eq!(hash1, hash2, "identical content is stored once");
        assert_eq!(size1, 10);
        let back = read(dir.path(), &key, &hash1).unwrap();
        assert_eq!(back, b"xray bytes");
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let dir = tempfile::tempdir().unwrap();
        let (hash, _) = store(dir.path(), &DataKey::generate(), b"secret").unwrap();
        assert!(read(dir.path(), &DataKey::generate(), &hash).is_err());
    }

    #[test]
    fn rejects_empty_and_oversized() {
        let dir = tempfile::tempdir().unwrap();
        let key = DataKey::generate();
        assert!(store(dir.path(), &key, b"").is_err());
        assert!(store(dir.path(), &key, &vec![0u8; MAX_BYTES + 1]).is_err());
    }
}
