pub mod client;
pub mod messages;
pub mod server;

use crate::client::KipawaTunnel;
use crate::server::KipawaServer;
use quinn::rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tracing::info;
use uuid::Uuid;

const SERVER_NAME: &str = "localhost";
const SERVER_PORT: u16 = 5001;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    let (cert_der, key) = generate_self_signed_cert()?;

    let server = KipawaServer::new_with_self_signed_certificate(cert_der.clone(), key).unwrap();

    info!("Starting server");

    server
        .listen(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            SERVER_PORT,
        ))
        .await?;

    info!("Server started");

    loop {
        let client = KipawaTunnel::connect_with_certificates(
            &Uuid::new_v4().to_string(),
            &Uuid::max().to_string(),
            "kipawa-test",
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), SERVER_PORT),
            SERVER_NAME.to_string(),
            vec![cert_der.clone()],
        )
        .await?;

        client.close();
    }

    Ok(())
}

fn generate_self_signed_cert()
-> anyhow::Result<(CertificateDer<'static>, PrivatePkcs8KeyDer<'static>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    let cert_der = CertificateDer::from(cert.cert);
    let key = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    Ok((cert_der, key))
}
