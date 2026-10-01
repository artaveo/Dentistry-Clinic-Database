//! Database key management for Artaveo Dental (ADR-03 / ADR-04).
//!
//! * The SQLCipher database is encrypted with a random 256-bit **data key**.
//! * On the clinic PC the data key is stored wrapped by **Windows DPAPI**
//!   (machine scope + application entropy), so the copied `.db` file is useless
//!   on another computer.
//! * The same data key is also wrapped with a **Recovery Key** that the Owner
//!   prints during setup. The recovery-wrapped blob travels with every backup,
//!   so a backup can be restored on a brand-new PC without internet.

mod protector;
mod recovery;

pub use protector::{default_protector, KeyProtector, ProtectorKind};
pub use recovery::RecoveryKey;

use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("recovery key is malformed: {0}")]
    MalformedRecoveryKey(&'static str),
    #[error("recovery key checksum mismatch (typo?)")]
    RecoveryChecksum,
    #[error("wrapped key blob is invalid or was produced with a different key")]
    Unwrap,
    #[error("OS key protection failed: {0}")]
    Os(String),
}

/// 256-bit SQLCipher data key. Zeroed on drop.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct DataKey([u8; 32]);

impl DataKey {
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut k = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut k);
        Self(k)
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Raw-key form understood by SQLCipher: `x'<64 hex>'`.
    /// Using a raw key skips SQLCipher's PBKDF2 step (the key is already random).
    pub fn sqlcipher_pragma_value(&self) -> String {
        format!("\"x'{}'\"", hex::encode(self.0))
    }
}

impl std::fmt::Debug for DataKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataKey(**redacted**)")
    }
}
