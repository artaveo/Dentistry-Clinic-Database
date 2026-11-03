use hmac::{Hmac, Mac};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};

const ID_CLIENT: &[u8] = b"artaveo-client";
const ID_SERVER: &[u8] = b"artaveo-server";

pub fn start_client(code: &str) -> (Spake2<Ed25519Group>, Vec<u8>) {
    Spake2::<Ed25519Group>::start_a(
        &Password::new(code.as_bytes()),
        &Identity::new(ID_CLIENT),
        &Identity::new(ID_SERVER),
    )
}

pub fn start_server(code: &str) -> (Spake2<Ed25519Group>, Vec<u8>) {
    Spake2::<Ed25519Group>::start_b(
        &Password::new(code.as_bytes()),
        &Identity::new(ID_CLIENT),
        &Identity::new(ID_SERVER),
    )
}

/// Key-confirmation MAC bound to the TLS certificate the *sender* saw.
pub fn confirm_mac(key: &[u8], role: &[u8], cert_fp: &[u8; 32]) -> Vec<u8> {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("any key length");
    m.update(b"artaveo-pair-confirm/v1\0");
    m.update(role);
    m.update(cert_fp);
    m.finalize().into_bytes().to_vec()
}

pub fn verify_mac(key: &[u8], role: &[u8], cert_fp: &[u8; 32], mac: &[u8]) -> bool {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("any key length");
    m.update(b"artaveo-pair-confirm/v1\0");
    m.update(role);
    m.update(cert_fp);
    m.verify_slice(mac).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_code_same_key_wrong_code_differs() {
        let (c, cm) = start_client("123456");
        let (s, sm) = start_server("123456");
        let kc = c.finish(&sm).unwrap();
        let ks = s.finish(&cm).unwrap();
        assert_eq!(kc, ks);

        let (c, cm) = start_client("123457");
        let (s, sm) = start_server("123456");
        assert_ne!(c.finish(&sm).unwrap(), s.finish(&cm).unwrap());
    }
}
