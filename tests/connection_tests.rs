use kipawa::{AuthKey, Error, IngressId, Server, Tunnel, TunnelName};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::OnceLock;

static CRYPTO: OnceLock<(CertificateDer<'static>, PrivatePkcs8KeyDer<'static>)> = OnceLock::new();

static SERVER_NAME: &'static str = "localhost";

fn init_crypto() -> &'static (CertificateDer<'static>, PrivatePkcs8KeyDer<'static>) {
    CRYPTO.get_or_init(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .unwrap();

        let cert = rcgen::generate_simple_self_signed(vec![SERVER_NAME.to_string()]).unwrap();
        let cert_der = CertificateDer::from(cert.cert);
        let key = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());

        (cert_der, key)
    })
}

#[tokio::test]
async fn test_handshake_v1_successful() {
    let (cert_der, key) = init_crypto();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();

    let server = Server::new_with_self_signed_certificate(
        auth_key.clone(),
        cert_der.clone(),
        key.clone_key(),
    )
    .unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
        .await
        .unwrap();

    let tunnel = Tunnel::connect_with_certificates(
        auth_key.clone(),
        IngressId::try_from("").unwrap(),
        TunnelName::try_from("").unwrap(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), server.port()),
        SERVER_NAME.to_string(),
        vec![cert_der.clone()],
    )
    .await
    .unwrap();

    tunnel.close();

    server.close().await;
}

#[tokio::test]
async fn test_handshake_v1_wrong_auth_key() {
    let (cert_der, key) = init_crypto();

    let server = Server::new_with_self_signed_certificate(
        AuthKey::try_from("valid_auth_key").unwrap(),
        cert_der.clone(),
        key.clone_key(),
    )
    .unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
        .await
        .unwrap();

    if let Err(e) = Tunnel::connect_with_certificates(
        AuthKey::try_from("wrong_auth_key").unwrap(),
        IngressId::try_from("").unwrap(),
        TunnelName::try_from("").unwrap(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), server.port()),
        SERVER_NAME.to_string(),
        vec![cert_der.clone()],
    )
    .await
    {
        matches!(e, Error::AuthKeyRejected);
        server.close().await;
    } else {
        server.close().await;
        panic!("Should not authenticate.");
    }
}
