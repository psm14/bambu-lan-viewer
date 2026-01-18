use rustls::client::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier, WebPkiVerifier,
};
use rustls::{
    Certificate, ClientConfig, DigitallySignedStruct, Error as RustlsError, ServerName,
    SignatureScheme,
};
use std::sync::Arc;

pub fn insecure_client_config() -> ClientConfig {
    let verifier = Arc::new(InsecureVerifier);
    ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth()
}

struct InsecureVerifier;

impl ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &Certificate,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &Certificate,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        WebPkiVerifier::verification_schemes()
    }
}
