use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::OnceLock,
};

use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use siffleux::{
    AuthKey, Client, Egress, Ingress, IngressClone, IngressId, Server, TcpEgress, TcpIngress,
    TunnelName, generate_self_signed_certificate,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{Level, info};

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

        rustls::crypto::ring::default_provider()
            .install_default()
            .unwrap();

        let (cert, key, cert_hash, _, _) = generate_self_signed_certificate(SERVER_NAME);

        (cert, key, cert_hash)
    })
}

#[tokio::test]
async fn test_send_and_receive_data() {
    let (cert_der, key, cert_hash) = init();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("111").unwrap();

    let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        auth_key.hash(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_ingress.start().await.unwrap();

    let Some(tcp_ingress_socket_addr) = tcp_ingress.socket_addr().unwrap() else {
        panic!("Shouldn't reach!!");
    };

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    let client = Client::new();

    let (tunnel, _) = client
        .connect_tunnel_with_certificate_hash(
            auth_key.clone(),
            ingress_id.clone(),
            TunnelName::try_from("aaa").unwrap(),
            server.address().unwrap(),
            SERVER_NAME.to_string(),
            cert_hash.clone(),
        )
        .await
        .unwrap();

    let tunnel_clone = tunnel.clone();

    tokio::spawn(async move {
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
                        write_stream.write(&mut buffer[..size]).await.unwrap();
                    }
                });
            }
        });

        let tcp_egress = TcpEgress::new(tunnel_clone, tcp_echo_addr);

        let _ = tcp_egress.start().await;
    });

    let mut stream = TcpStream::connect(tcp_ingress_socket_addr).await.unwrap();

    let mut buffer = [0u8; 32];

    stream.write_all(b"Hello, server!").await.unwrap();
    let size = stream.read(&mut buffer).await.unwrap();

    assert_eq!(
        "Hello, server!",
        &String::from_utf8(buffer[..size].to_vec()).unwrap()
    );

    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_target_tcp_write_dropped() {
    let (cert_der, key, cert_hash) = init();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("ingress").unwrap();

    let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        auth_key.hash(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_ingress.start().await.unwrap();

    let Some(tcp_ingress_socket_addr) = tcp_ingress.socket_addr().unwrap() else {
        panic!("Shouldn't reach!!");
    };

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    let client = Client::new();

    let (tunnel, _) = client
        .connect_tunnel_with_certificate_hash(
            auth_key.clone(),
            ingress_id.clone(),
            TunnelName::try_from("aaa").unwrap(),
            server.address().unwrap(),
            SERVER_NAME.to_string(),
            cert_hash.clone(),
        )
        .await
        .unwrap();

    let tunnel_clone = tunnel.clone();

    tokio::spawn(async move {
        let tcp_echo = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let tcp_echo_addr = tcp_echo.local_addr().unwrap();

        tokio::spawn(async move {
            while let Ok((tcp_stream, _)) = tcp_echo.accept().await {
                drop(tcp_stream);
            }
        });

        let tcp_egress = TcpEgress::new(tunnel_clone, tcp_echo_addr);

        let _ = tcp_egress.start().await;
    });

    let mut stream = TcpStream::connect(tcp_ingress_socket_addr).await.unwrap();

    stream.write(&mut [0u8; 10]).await.unwrap();
    let result = stream.read(&mut [0u8; 0]).await;

    assert_eq!(false, result.is_err());
    assert_eq!(Some(0), result.ok());

    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_origin_tcp_write_dropped() {
    let (cert_der, key, cert_hash) = init();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("ingress").unwrap();

    let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        auth_key.hash(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_ingress.start().await.unwrap();

    let Some(tcp_ingress_socket_addr) = tcp_ingress.socket_addr().unwrap() else {
        panic!("Shouldn't reach!!");
    };

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    let client = Client::new();

    let (tunnel, _) = client
        .connect_tunnel_with_certificate_hash(
            auth_key.clone(),
            ingress_id.clone(),
            TunnelName::try_from("aaa").unwrap(),
            server.address().unwrap(),
            SERVER_NAME.to_string(),
            cert_hash.clone(),
        )
        .await
        .unwrap();

    let tunnel_clone = tunnel.clone();

    tokio::spawn(async move {
        let tcp_echo = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let tcp_echo_addr = tcp_echo.local_addr().unwrap();

        tokio::spawn(async move {
            while let Ok((mut tcp_stream, _)) = tcp_echo.accept().await {
                let _ = tcp_stream.read_f32().await;
            }
        });

        let tcp_egress = TcpEgress::new(tunnel_clone, tcp_echo_addr);

        let _ = tcp_egress.start().await;
    });

    let tcp_stream = TcpStream::connect(tcp_ingress_socket_addr).await.unwrap();

    let tcp_socket_addr = tcp_stream.local_addr().unwrap();
    let (mut tcp_read_stream, tcp_write_stream) = tcp_stream.into_split();

    drop(tcp_write_stream);

    info!("Dropped tcp {tcp_socket_addr} write stream");

    let result = tcp_read_stream.read(&mut [0u8; 0]).await;

    assert_eq!(false, result.is_err());
    assert_eq!(Some(0), result.ok());

    tunnel.close().await;
    server.stop().await.unwrap();
}
