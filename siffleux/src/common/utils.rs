use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use sha2::{Digest, Sha256};

pub fn generate_self_signed_certificate(
    subject_alt_name: &str,
) -> (
    CertificateDer<'static>,
    PrivatePkcs8KeyDer<'static>,
    Vec<u8>,
) {
    let self_signed =
        rcgen::generate_simple_self_signed(vec![subject_alt_name.to_string()]).unwrap();
    let cert_der = CertificateDer::from(self_signed.cert);
    let key = PrivatePkcs8KeyDer::from(self_signed.signing_key.serialize_der());
    let cert_hash = Sha256::digest(cert_der.as_ref());

    (cert_der, key, cert_hash.to_vec())
}
