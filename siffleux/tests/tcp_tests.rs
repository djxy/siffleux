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
    TcpEgress, TcpIngress, authentication::V1CertifcateHash, generate_self_signed_certificate,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{Level, info};
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

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        auth_key.clone(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_ingress.start().await.unwrap();

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    let tcp_echo = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();
    let tcp_echo_addr = tcp_echo.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((tcp_stream, _)) = tcp_echo.accept().await {
            tokio::spawn(async move {
                let mut buffer = [0u8, 32];
                let (mut read_stream, mut write_stream) = tcp_stream.into_split();

                while let Ok(size) = read_stream.read(&mut buffer).await {
                    if size == 0 {
                        return;
                    }

                    write_stream.write(&mut buffer[..size]).await.unwrap();
                }
            });
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

    let tcp_egress = TcpEgress::new(
        egress_id,
        Box::new(authentication),
        ingress_id,
        tcp_echo_addr,
    );

    tcp_egress.start().await.unwrap();

    let mut stream = TcpStream::connect(tcp_ingress.socket_addr().unwrap())
        .await
        .unwrap();

    let mut buffer = [0u8; 14];

    stream.write_all(b"Hello, server!").await.unwrap();
    stream.read_exact(&mut buffer).await.unwrap();

    stream.shutdown().await.unwrap();

    assert_eq!(0, stream.read_to_end(&mut Vec::new()).await.unwrap());
    assert_eq!(
        "Hello, server!",
        &String::from_utf8(buffer[..].to_vec()).unwrap()
    );

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_target_tcp_write_dropped() {
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

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        auth_key.clone(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_ingress.start().await.unwrap();

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    let tcp_echo = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();
    let tcp_echo_addr = tcp_echo.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((tcp_stream, _)) = tcp_echo.accept().await {
            drop(tcp_stream);
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

    let tcp_egress = TcpEgress::new(
        egress_id,
        Box::new(authentication),
        ingress_id,
        tcp_echo_addr,
    );

    tcp_egress.start().await.unwrap();

    let mut stream = TcpStream::connect(tcp_ingress.socket_addr().unwrap())
        .await
        .unwrap();

    stream.write(&mut [0u8; 10]).await.unwrap();
    let result = stream.read(&mut [0u8; 0]).await;

    assert_eq!(false, result.is_err());
    assert_eq!(Some(0), result.ok());

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_origin_tcp_write_dropped() {
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

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        auth_key.clone(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_ingress.start().await.unwrap();

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    let tcp_echo = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();
    let tcp_echo_addr = tcp_echo.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut tcp_stream, _)) = tcp_echo.accept().await {
            let _ = tcp_stream.read_f32().await;
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

    let tcp_egress = TcpEgress::new(
        egress_id,
        Box::new(authentication),
        ingress_id,
        tcp_echo_addr,
    );

    tcp_egress.start().await.unwrap();

    let tcp_stream = TcpStream::connect(tcp_ingress.socket_addr().unwrap())
        .await
        .unwrap();

    let tcp_socket_addr = tcp_stream.local_addr().unwrap();
    let (mut tcp_read_stream, tcp_write_stream) = tcp_stream.into_split();

    drop(tcp_write_stream);

    info!("Dropped tcp {tcp_socket_addr} write stream");

    let result = tcp_read_stream.read(&mut [0u8; 0]).await;

    assert_eq!(false, result.is_err());
    assert_eq!(Some(0), result.ok());

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

    let tcp_egress = TcpEgress::new(
        egress_id,
        Box::new(authentication),
        ingress_id,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_egress.start().await.unwrap();

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
