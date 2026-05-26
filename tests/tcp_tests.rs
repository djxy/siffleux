use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::OnceLock,
    time::Duration,
};

use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use siffleux::{
    AuthKey, IngressId, Server, Tunnel, TunnelName,
    codes::CLOSED,
    egress::Egress,
    ingress::{Ingress, IngressClone},
    tcp_egress::TcpEgress,
    tcp_ingress::TcpIngress,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::sleep,
};

static SERVER_NAME: &'static str = "localhost";

static CRYPTO: OnceLock<(CertificateDer<'static>, PrivatePkcs8KeyDer<'static>)> = OnceLock::new();

fn init_crypto() -> &'static (CertificateDer<'static>, PrivatePkcs8KeyDer<'static>) {
    CRYPTO.get_or_init(|| {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

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
async fn test_send_and_receive_data() {
    let (cert_der, key) = init_crypto();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("111").unwrap();

    let server = Server::new_with_self_signed_certificate(
        auth_key.clone(),
        cert_der.clone(),
        key.clone_key(),
    )
    .unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_ingress.start().await.unwrap();

    let Some(tcp_ingress_socket_addr) = tcp_ingress.socket_addr().unwrap() else {
        panic!("Shouldn't reach!!");
    };

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    let tunnel = Tunnel::connect_to_server_with_certificates(
        auth_key.clone(),
        ingress_id.clone(),
        TunnelName::try_from("aaa").unwrap(),
        server.address().unwrap(),
        SERVER_NAME.to_string(),
        vec![cert_der.clone()],
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

    server.close().await.unwrap();
}

#[tokio::test]
async fn test_target_tcp_write_dropped() {
    let (cert_der, key) = init_crypto();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("ingress").unwrap();

    let server = Server::new_with_self_signed_certificate(
        auth_key.clone(),
        cert_der.clone(),
        key.clone_key(),
    )
    .unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_ingress.start().await.unwrap();

    let Some(tcp_ingress_socket_addr) = tcp_ingress.socket_addr().unwrap() else {
        panic!("Shouldn't reach!!");
    };

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    let tunnel = Tunnel::connect_to_server_with_certificates(
        auth_key.clone(),
        ingress_id.clone(),
        TunnelName::try_from("").unwrap(),
        server.address().unwrap(),
        SERVER_NAME.to_string(),
        vec![cert_der.clone()],
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

    server.close().await.unwrap();
}

#[tokio::test]
async fn test_origin_tcp_write_dropped() {
    let (cert_der, key) = init_crypto();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("ingress").unwrap();

    let server = Server::new_with_self_signed_certificate(
        auth_key.clone(),
        cert_der.clone(),
        key.clone_key(),
    )
    .unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_ingress.start().await.unwrap();

    let Some(tcp_ingress_socket_addr) = tcp_ingress.socket_addr().unwrap() else {
        panic!("Shouldn't reach!!");
    };

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    let tunnel = Tunnel::connect_to_server_with_certificates(
        auth_key.clone(),
        ingress_id.clone(),
        TunnelName::try_from("").unwrap(),
        server.address().unwrap(),
        SERVER_NAME.to_string(),
        vec![cert_der.clone()],
    )
    .await
    .unwrap();

    let (mut tcp_read_stream, tcp_write_stream) = TcpStream::connect(tcp_ingress_socket_addr)
        .await
        .unwrap()
        .into_split();

    drop(tcp_write_stream);

    let result = tcp_read_stream.read(&mut [0u8; 0]).await;

    assert_eq!(false, result.is_err());
    assert_eq!(Some(0), result.ok());

    tunnel.close(&CLOSED);
    server.close().await.unwrap();
}
