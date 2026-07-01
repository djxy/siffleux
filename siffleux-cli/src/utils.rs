use base64::{
    Engine,
    alphabet::{self},
    engine::{
        self,
        general_purpose::{self},
    },
};
use rustls_pki_types::{CertificateDer, PrivatePkcs8KeyDer, pem::PemObject};
use siffleux::{generate_self_signed_certificate, hash_certificate};
use tokio::signal::unix::{SignalKind, signal};
use tracing::info;

pub const BASE64_ENGINE: engine::GeneralPurpose =
    engine::GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::PAD);

const SIFFLEUX_CERT_FILE: &'static str = "siffleux-cert.pem";
const SIFFLEUX_KEY_FILE: &'static str = "siffleux-key.pem";

pub async fn load_or_generate_self_signed_certificate(
    cert_subject_alt_name: &str,
) -> (
    CertificateDer<'static>,
    PrivatePkcs8KeyDer<'static>,
    Vec<u8>,
) {
    let cert_file_res = rustls_pki_types::CertificateDer::from_pem_file(SIFFLEUX_CERT_FILE);
    let key_file_res = rustls_pki_types::PrivatePkcs8KeyDer::from_pem_file(SIFFLEUX_KEY_FILE);

    let (cert, key, cert_hash) = if let Ok(cert) = cert_file_res
        && let Ok(key) = key_file_res
    {
        let cert_hash = hash_certificate(&cert);

        info!("Loaded self signed certificate");

        (cert, key, cert_hash)
    } else {
        let (cert, key, cert_hash, cert_pem, key_pem) =
            generate_self_signed_certificate(cert_subject_alt_name);

        info!("Created self signed certificate");

        tokio::fs::write(SIFFLEUX_CERT_FILE, cert_pem)
            .await
            .unwrap();
        tokio::fs::write(SIFFLEUX_KEY_FILE, key_pem).await.unwrap();

        (cert, key, cert_hash)
    };

    info!("Certificate hash: {}", BASE64_ENGINE.encode(&cert_hash));

    return (cert, key, cert_hash);
}

pub fn generate_secure_random_key<const L: usize>() -> String {
    let mut bytes = [0u8; L];

    getrandom::fill(&mut bytes).unwrap();

    BASE64_ENGINE.encode(bytes)
}

pub async fn wait_for_shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    let sigterm = async {
        signal(SignalKind::terminate())
            .expect("Failed to listen SIGTERM signal.")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = sigterm => {},
    }
}
