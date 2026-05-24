use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::OnceLock,
};

use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use siffleux::{
    AuthKey, IngressId, Server, Tunnel, TunnelName,
    ingress::{Ingress, IngressClone},
    tcp_ingress::TcpIngress,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

static SERVER_NAME: &'static str = "localhost";

static CRYPTO: OnceLock<(CertificateDer<'static>, PrivatePkcs8KeyDer<'static>)> = OnceLock::new();

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
async fn test_send_and_receive_data() {
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
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
        .await
        .unwrap();

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
    );

    tcp_ingress.start().await.unwrap();

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    let tunnel = Tunnel::connect_to_server_with_certificates(
        auth_key.clone(),
        ingress_id.clone(),
        TunnelName::try_from("").unwrap(),
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            server.address().unwrap().port(),
        ),
        SERVER_NAME.to_string(),
        vec![cert_der.clone()],
    )
    .await
    .unwrap();

    let Some(tcp_ingress_socket_addr) = tcp_ingress.socket_addr().unwrap() else {
        panic!("Shouldn't reach!!");
    };

    let tunnel_receive = tunnel.clone();

    tokio::spawn(async move {
        let mut buffer = [0u8; 32];
        let (mut read_channel, mut write_channel) = tunnel_receive.accept_stream().await.unwrap();

        let size = read_channel.read(&mut buffer).await.unwrap().unwrap();

        write_channel.write(&mut buffer[..size]).await.unwrap();
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
