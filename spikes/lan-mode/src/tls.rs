use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{ring as provider, verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

/// Self-signed identity of one clinic server. Generated once at
/// "this computer is the clinic server" and stored in the app data folder.
pub struct ServerIdentity {
    pub cert: CertificateDer<'static>,
    pub key: PrivatePkcs8KeyDer<'static>,
}

impl ServerIdentity {
    pub fn generate(clinic_name: &str) -> anyhow::Result<Self> {
        let mut params = rcgen::CertificateParams::new(vec!["artaveo-server.local".into()])?;
        params.distinguished_name.push(rcgen::DnType::CommonName, clinic_name);
        // Long-lived: trust comes from pinning, not from expiry/CA.
        params.not_after = rcgen::date_time_ymd(2075, 1, 1);
        let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
        let cert = params.self_signed(&key)?;
        Ok(Self { cert: cert.der().clone(), key: PrivatePkcs8KeyDer::from(key.serialize_der()) })
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(&self.cert)
    }

    pub fn server_config(&self) -> anyhow::Result<Arc<rustls::ServerConfig>> {
        let cfg = rustls::ServerConfig::builder_with_provider(Arc::new(provider::default_provider()))
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(vec![self.cert.clone()], PrivateKeyDer::Pkcs8(self.key.clone_key()))?;
        Ok(Arc::new(cfg))
    }
}

pub fn fingerprint(cert: &CertificateDer<'_>) -> [u8; 32] {
    Sha256::digest(cert.as_ref()).into()
}

/// Verifier used by clients.
/// * `Pinned(fp)` — normal operation: only the paired server's cert is accepted.
/// * `CaptureForPairing` — before pairing: accept any cert, remember its fp;
///   the SPAKE2 confirmation then proves that fp belongs to the real server.
#[derive(Debug)]
pub struct ClinicVerifier {
    mode: Mode,
    seen: Mutex<Option<[u8; 32]>>,
    provider: Arc<CryptoProvider>,
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Pinned([u8; 32]),
    CaptureForPairing,
}

impl ClinicVerifier {
    pub fn pinned(fp: [u8; 32]) -> Arc<Self> {
        Arc::new(Self {
            mode: Mode::Pinned(fp),
            seen: Mutex::new(None),
            provider: Arc::new(provider::default_provider()),
        })
    }
    pub fn capture() -> Arc<Self> {
        Arc::new(Self {
            mode: Mode::CaptureForPairing,
            seen: Mutex::new(None),
            provider: Arc::new(provider::default_provider()),
        })
    }
    pub fn seen_fingerprint(&self) -> Option<[u8; 32]> {
        *self.seen.lock().unwrap()
    }
    pub fn client_config(self: &Arc<Self>) -> anyhow::Result<Arc<rustls::ClientConfig>> {
        let cfg = rustls::ClientConfig::builder_with_provider(self.provider.clone())
            .with_safe_default_protocol_versions()?
            .dangerous()
            .with_custom_certificate_verifier(self.clone())
            .with_no_client_auth();
        Ok(Arc::new(cfg))
    }
}

impl ServerCertVerifier for ClinicVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fp = fingerprint(end_entity);
        *self.seen.lock().unwrap() = Some(fp);
        match self.mode {
            Mode::CaptureForPairing => Ok(ServerCertVerified::assertion()),
            Mode::Pinned(expected) if expected == fp => Ok(ServerCertVerified::assertion()),
            Mode::Pinned(_) => Err(rustls::Error::General(
                "server certificate does not match the paired clinic server".into(),
            )),
        }
    }

    // Signatures are still verified: proves the server holds the cert's key.
    fn verify_tls12_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}
