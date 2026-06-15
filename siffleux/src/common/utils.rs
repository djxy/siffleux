use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use sha2::{Digest, Sha256};

pub fn generate_self_signed_certificate(
    subject_alt_name: &str,
) -> (
    CertificateDer<'static>,
    PrivatePkcs8KeyDer<'static>,
    Vec<u8>,
    String, // Cert PEM
    String, // Key PEM
) {
    let self_signed =
        rcgen::generate_simple_self_signed(vec![subject_alt_name.to_string()]).unwrap();
    let cert_pem = self_signed.cert.pem();
    let key_pem = self_signed.signing_key.serialize_pem();
    let cert_der = CertificateDer::from(self_signed.cert);
    let key = PrivatePkcs8KeyDer::from(self_signed.signing_key.serialize_der());
    let cert_hash = hash_certificate(&cert_der);

    (cert_der, key, cert_hash, cert_pem, key_pem)
}

pub fn hash_certificate(certificate: &CertificateDer) -> Vec<u8> {
    Sha256::digest(certificate.as_ref()).to_vec()
}
