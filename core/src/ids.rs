//! Identifiers (ADR-05): UUIDv7 primary keys.

use sha2::{Digest, Sha256};

/// Time-ordered UUIDv7 for new rows.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Deterministic UUIDv7-layout id for system seed rows (reference items,
/// provinces, districts, system roles): fixed timestamp 2026-10-01T00:00:00Z
/// and 74 bits from SHA-256 of a namespaced code. The same seed row therefore
/// has the same id in every clinic database (important for future sync/import).
pub fn seed_id(namespace: &str, code: &str) -> String {
    const SEED_EPOCH_MS: u64 = 1_790_812_800_000; // 2026-10-01T00:00:00Z
    let h = Sha256::digest(format!("artaveo-seed/v1\0{namespace}\0{code}").as_bytes());
    let mut b = [0u8; 16];
    b[..6].copy_from_slice(&SEED_EPOCH_MS.to_be_bytes()[2..]);
    b[6..].copy_from_slice(&h[..10]);
    b[6] = 0x70 | (b[6] & 0x0F); // version 7
    b[8] = 0x80 | (b[8] & 0x3F); // RFC 4122 variant
    uuid::Uuid::from_bytes(b).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_ids_are_stable_valid_v7_and_distinct() {
        let a = seed_id("province", "AF-KAB");
        assert_eq!(a, seed_id("province", "AF-KAB"));
        assert_ne!(a, seed_id("province", "AF-BAL"));
        assert_ne!(a, seed_id("district", "AF-KAB"));
        let u = uuid::Uuid::parse_str(&a).unwrap();
        assert_eq!(u.get_version_num(), 7);
    }

    #[test]
    fn new_ids_sort_by_time() {
        let a = new_id();
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(a < new_id());
    }
}
