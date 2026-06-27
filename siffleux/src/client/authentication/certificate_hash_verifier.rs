use rustls::{
    DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature},
    pki_types::CertificateDer,
};
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub struct CertificateHashVerifier {
    certificate_hash: Vec<u8>,
    supported_signature_schemes: Vec<SignatureScheme>,
}

impl CertificateHashVerifier {
    pub fn new(certificate_hash: Vec<u8>) -> Self {
        Self {
            certificate_hash,
            supported_signature_schemes: CryptoProvider::get_default()
                .unwrap()
                .signature_verification_algorithms
                .supported_schemes(),
        }
    }
}

impl ServerCertVerifier for CertificateHashVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let got = Sha256::digest(end_entity.as_ref()).to_vec();
        if got == self.certificate_hash {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("certificate hash mismatch".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &CryptoProvider::get_default()
                .unwrap()
                .signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &CryptoProvider::get_default()
                .unwrap()
                .signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.supported_signature_schemes.clone()
    }
}
