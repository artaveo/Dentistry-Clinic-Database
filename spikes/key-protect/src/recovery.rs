use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use data_encoding::BASE32_NOPAD;
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{DataKey, KeyError};

const SECRET_LEN: usize = 20; // 160 bits of entropy
const CHECK_LEN: usize = 2;
const BLOB_MAGIC: &[u8; 4] = b"ARK1";
const HKDF_INFO: &[u8] = b"artaveo-dental/recovery-wrap/v1";

/// Human-friendly Recovery Key, e.g. `K3QZ7M-...` (6 groups of 6 Base32 chars).
///
/// 160 random bits + 16-bit checksum so typos are detected before attempting
/// a restore. High entropy means no slow KDF is required.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct RecoveryKey([u8; SECRET_LEN]);

impl RecoveryKey {
    pub fn generate() -> Self {
        let mut s = [0u8; SECRET_LEN];
        rand::rngs::OsRng.fill_bytes(&mut s);
        Self(s)
    }

    fn checksum(secret: &[u8]) -> [u8; CHECK_LEN] {
        let d = Sha256::digest(secret);
        [d[0], d[1]]
    }

    /// Printable form: 36 Base32 characters in groups of 6.
    pub fn to_display(&self) -> String {
        let mut raw = Vec::with_capacity(SECRET_LEN + CHECK_LEN);
        raw.extend_from_slice(&self.0);
        raw.extend_from_slice(&Self::checksum(&self.0));
        let enc = BASE32_NOPAD.encode(&raw);
        raw.zeroize();
        enc.as_bytes().chunks(6).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("-")
    }

    /// Accepts the printed form; tolerant of case, spaces, dashes and the
    /// usual OCR/typing confusions (O→0 is *not* valid Base32, so map 0→O, 1→I, 8→B).
    pub fn parse(input: &str) -> Result<Self, KeyError> {
        let cleaned: String = input
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .map(|c| match c.to_ascii_uppercase() {
                '0' => 'O',
                '1' => 'I',
                '8' => 'B',
                o => o,
            })
            .collect();
        let raw = BASE32_NOPAD
            .decode(cleaned.as_bytes())
            .map_err(|_| KeyError::MalformedRecoveryKey("not base32"))?;
        if raw.len() != SECRET_LEN + CHECK_LEN {
            return Err(KeyError::MalformedRecoveryKey("wrong length"));
        }
        let (secret, check) = raw.split_at(SECRET_LEN);
        if Self::checksum(secret) != check {
            return Err(KeyError::RecoveryChecksum);
        }
        let mut s = [0u8; SECRET_LEN];
        s.copy_from_slice(secret);
        Ok(Self(s))
    }

    fn kek(&self, salt: &[u8]) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(Some(salt), &self.0);
        let mut okm = [0u8; 32];
        hk.expand(HKDF_INFO, &mut okm).expect("32 bytes is a valid HKDF length");
        okm
    }

    /// Blob layout: `ARK1 | salt(16) | nonce(24) | ciphertext+tag(48)`.
    pub fn wrap(&self, key: &DataKey) -> Vec<u8> {
        let mut salt = [0u8; 16];
        let mut nonce = [0u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let mut kek = self.kek(&salt);
        let cipher = XChaCha20Poly1305::new((&kek).into());
        kek.zeroize();
        let ct = cipher
            .encrypt(XNonce::from_slice(&nonce), key.as_bytes().as_slice())
            .expect("encryption with valid key cannot fail");
        let mut out = Vec::with_capacity(4 + 16 + 24 + ct.len());
        out.extend_from_slice(BLOB_MAGIC);
        out.extend_from_slice(&salt);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        out
    }

    pub fn unwrap(&self, blob: &[u8]) -> Result<DataKey, KeyError> {
        if blob.len() != 4 + 16 + 24 + 32 + 16 || &blob[..4] != BLOB_MAGIC {
            return Err(KeyError::Unwrap);
        }
        let salt = &blob[4..20];
        let nonce = &blob[20..44];
        let mut kek = self.kek(salt);
        let cipher = XChaCha20Poly1305::new((&kek).into());
        kek.zeroize();
        let mut pt = cipher.decrypt(XNonce::from_slice(nonce), &blob[44..]).map_err(|_| KeyError::Unwrap)?;
        let mut k = [0u8; 32];
        k.copy_from_slice(&pt);
        pt.zeroize();
        Ok(DataKey::from_bytes(k))
    }
}

impl std::fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryKey(**redacted**)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_roundtrip_and_format() {
        let rk = RecoveryKey::generate();
        let shown = rk.to_display();
        assert_eq!(shown.len(), 36 + 5, "6 groups of 6 + 5 dashes: {shown}");
        let parsed = RecoveryKey::parse(&shown.to_lowercase().replace('-', " ")).unwrap();
        assert_eq!(parsed.0, rk.0);
    }

    #[test]
    fn typo_is_detected_by_checksum() {
        let rk = RecoveryKey::generate();
        let mut shown: Vec<char> = rk.to_display().chars().collect();
        shown[3] = if shown[3] == 'A' { 'B' } else { 'A' };
        let typo: String = shown.into_iter().collect();
        assert!(matches!(
            RecoveryKey::parse(&typo),
            Err(KeyError::RecoveryChecksum) | Err(KeyError::MalformedRecoveryKey(_))
        ));
    }

    #[test]
    fn wrap_unwrap() {
        let rk = RecoveryKey::generate();
        let dk = DataKey::generate();
        let blob = rk.wrap(&dk);
        assert_eq!(rk.unwrap(&blob).unwrap(), dk);
        let other = RecoveryKey::generate();
        assert!(matches!(other.unwrap(&blob), Err(KeyError::Unwrap)));
    }
}
