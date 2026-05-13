pub mod server;
mod tunnel;

use crate::server::TunnelServer;
use crate::tunnel::Tunnel;
use quinn::rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::time::sleep;
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

    let server = TunnelServer::new_with_self_signed_certificate(cert_der.clone(), key).unwrap();

    info!("Starting server");

    server
        .listen(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            SERVER_PORT,
        ))
        .await?;

    info!("Server started");

    loop {
        let client = Tunnel::new_with_self_signed_certificate(
            Uuid::max(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), SERVER_PORT),
            SERVER_NAME.to_string(),
            cert_der.clone(),
        )?;

        client.connect().await?;
        client.close().await;

        // sleep(Duration::from_millis(200)).await;
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
