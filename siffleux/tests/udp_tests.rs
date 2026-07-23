mod mock_ingress;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::OnceLock,
};

use rustls::{
    crypto::aws_lc_rs,
    pki_types::{CertificateDer, PrivatePkcs8KeyDer},
};
use siffleux::{
    AuthKey, Client, Egress, EgressId, Ingress, IngressClone, IngressId, Server, ServerId,
    UdpEgress, UdpIngress, authentication::V1CertifcateHash, generate_self_signed_certificate,
};
use tokio::net::UdpSocket;
use tracing::Level;
use uuid::Uuid;

use crate::mock_ingress::MockIngress;

static SERVER_NAME: &'static str = "localhost";

static INIT: OnceLock<(
    CertificateDer<'static>,
    PrivatePkcs8KeyDer<'static>,
    Vec<u8>,
)> = OnceLock::new();

fn init() -> &'static (
    CertificateDer<'static>,
    PrivatePkcs8KeyDer<'static>,
    Vec<u8>,
) {
    INIT.get_or_init(|| {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(Level::DEBUG)
            .try_init();

        aws_lc_rs::default_provider()
            .install_default()
            .expect("Failed to install crypto provider");

        let (cert, key, cert_hash, _, _) = generate_self_signed_certificate(SERVER_NAME);

        (cert, key, cert_hash)
    })
}

#[tokio::test]
async fn test_send_and_receive_data() {
    let (cert_der, key, cert_hash) = init();
    let server_id = ServerId::try_from("server_id").unwrap();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("ingress").unwrap();
    let egress_id = EgressId::try_from("egress").unwrap();

    let server = Server::new_with_certificate(
        server_id,
        cert_der.clone(),
        key.clone_key(),
        cert_hash.clone(),
    )
    .unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();

    let udp_ingress = UdpIngress::new(
        ingress_id.clone(),
        auth_key.clone(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    udp_ingress.start().await.unwrap();

    server.assign_ingress(udp_ingress.clone_box()).unwrap();

    let udp_echo = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();
    let udp_echo_addr = udp_echo.local_addr().unwrap();

    tokio::spawn(async move {
        let mut buffer = [0u8; 32];
        while let Ok((len, origin)) = udp_echo.recv_from(&mut buffer).await {
            udp_echo.send_to(&buffer[..len], origin).await.unwrap();
        }
    });

    let client = Client::new();

    let authentication = V1CertifcateHash::new(
        client.clone(),
        auth_key,
        server.address().unwrap(),
        SERVER_NAME.to_string(),
        cert_hash.clone(),
    );

    let udp_egress = UdpEgress::new(
        egress_id,
        Box::new(authentication),
        ingress_id,
        udp_echo_addr,
    );

    udp_egress.start().await.unwrap();

    let udp_socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();

    udp_socket
        .connect(udp_ingress.get_socket_addr().await.unwrap())
        .await
        .unwrap();

    let mut buffer = [0u8; 14];

    udp_socket.send(b"Hello, server!").await.unwrap();
    udp_socket.recv(&mut buffer).await.unwrap();

    assert_eq!(
        "Hello, server!",
        &String::from_utf8(buffer[..].to_vec()).unwrap()
    );

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_tunnel_reconnection() {
    let (cert_der, key, cert_hash) = init();
    let server_id = ServerId::try_from("server_id").unwrap();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("ingress").unwrap();
    let egress_id = EgressId::try_from("egress").unwrap();

    let server = Server::new_with_certificate(
        server_id,
        cert_der.clone(),
        key.clone_key(),
        cert_hash.clone(),
    )
    .unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();

    let mock_ingress = MockIngress::new(ingress_id.clone(), auth_key.clone());
    let mut server_tunnel_subscriber = mock_ingress.subscribe_tunnel();

    mock_ingress.start().await.unwrap();

    server.assign_ingress(mock_ingress.clone_box()).unwrap();

    let client = Client::new();

    let authentication = V1CertifcateHash::new(
        client.clone(),
        auth_key,
        server.address().unwrap(),
        SERVER_NAME.to_string(),
        cert_hash.clone(),
    );

    let udp_egress = UdpEgress::new(
        egress_id,
        Box::new(authentication),
        ingress_id,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    udp_egress.start().await.unwrap();

    let mut previous_tunnel_id = Uuid::nil();

    for _ in 0..3 {
        let server_tunnel = server_tunnel_subscriber.recv().await.unwrap();

        assert_ne!(previous_tunnel_id, server_tunnel.id());

        previous_tunnel_id = server_tunnel.id();

        server_tunnel.close().await;
    }

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}
