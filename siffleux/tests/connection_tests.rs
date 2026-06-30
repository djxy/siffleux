mod mock_ingress;

use async_trait::async_trait;
use rustls::crypto::aws_lc_rs;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use siffleux::IngressClone;
use siffleux::authentication::{Authentication, V1CertifcateHash};
use siffleux::{
    AuthKey, Client, Egress, EgressId, Error, IngressId, Server, ServerId,
    generate_self_signed_certificate,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::sleep;
use tracing::Level;

use crate::mock_ingress::MockIngress;

static INIT: OnceLock<(
    CertificateDer<'static>,
    PrivatePkcs8KeyDer<'static>,
    Vec<u8>,
)> = OnceLock::new();

static SERVER_NAME: &'static str = "localhost";

#[derive(Clone)]
struct MockEgress {
    inner: Arc<MockEgressInner>,
}

struct MockEgressInner {
    id: EgressId,
    ingress_id: IngressId,
}

impl MockEgress {
    fn new(id: EgressId, ingress_id: IngressId) -> Self {
        Self {
            inner: Arc::new(MockEgressInner { id, ingress_id }),
        }
    }
}

#[async_trait]
impl Egress for MockEgress {
    fn id(&self) -> &EgressId {
        &self.inner.id
    }

    fn ingress_id(&self) -> &IngressId {
        &self.inner.ingress_id
    }

    async fn start(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        Ok(())
    }
}

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
async fn test_detect_tunnel_closed() {
    let (cert_der, key, cert_hash) = init();
    let server_id = ServerId::try_from("server_id").unwrap();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("ingress").unwrap();

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

    server.assign_ingress(mock_ingress.clone_box()).unwrap();

    let client = Client::new();

    let authentication = V1CertifcateHash::new(
        client.clone(),
        auth_key,
        server.address().unwrap(),
        SERVER_NAME.to_string(),
        cert_hash.clone(),
    );

    let mock_egress = MockEgress::new(EgressId::try_from("egress").unwrap(), ingress_id);

    let client_tunnel = authentication.connect(&mock_egress).await.unwrap();
    let server_tunnel = server_tunnel_subscriber.recv().await.unwrap();

    let server_tunnel_close_handle = tokio::spawn(async move {
        client_tunnel.closed().await;
    });

    let client_tunnel_close_handle = tokio::spawn(async move {
        server_tunnel.closed().await;
    });

    server.stop().await.unwrap();

    server_tunnel_close_handle.await.unwrap();
    client_tunnel_close_handle.await.unwrap();
}

#[tokio::test]
async fn test_send_data_over_stream() {
    let (cert_der, key, cert_hash) = init();
    let server_id = ServerId::try_from("server_id").unwrap();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("ingress").unwrap();

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

    server.assign_ingress(mock_ingress.clone_box()).unwrap();

    let client = Client::new();

    let authentication = V1CertifcateHash::new(
        client.clone(),
        auth_key,
        server.address().unwrap(),
        SERVER_NAME.to_string(),
        cert_hash.clone(),
    );

    let mock_egress = MockEgress::new(EgressId::try_from("egress").unwrap(), ingress_id);

    let client_tunnel = authentication.connect(&mock_egress).await.unwrap();
    let server_tunnel = server_tunnel_subscriber.recv().await.unwrap();

    const VALUE: u64 = 6329282199514132237;

    let client_tunnel_handle = tokio::spawn(async move {
        let mut client_buffer = [0u8; 8];
        let (mut client_read_stream, mut client_write_stream, _) =
            client_tunnel.accept_stream().await.unwrap();
        let size = client_read_stream
            .read_exact(&mut client_buffer[..])
            .await
            .unwrap();

        let client_value_received = u64::from_be_bytes(client_buffer[..size].try_into().unwrap());

        client_write_stream
            .write_u64(client_value_received)
            .await
            .unwrap();

        assert_eq!(8, client_tunnel.byte_counter().bytes_read());
        assert_eq!(8, client_tunnel.byte_counter().bytes_write());

        client_value_received
    });

    let mut server_buffer = [0u8; 8];
    let (mut server_read_stream, mut server_write_stream, _) =
        server_tunnel.create_stream().await.unwrap();

    server_write_stream
        .write(&mut VALUE.to_be_bytes())
        .await
        .unwrap();

    let size = server_read_stream
        .read_exact(&mut server_buffer[..])
        .await
        .unwrap();

    let server_value_received = u64::from_be_bytes(server_buffer[..size].try_into().unwrap());

    assert_eq!(8, server_tunnel.byte_counter().bytes_read());
    assert_eq!(8, server_tunnel.byte_counter().bytes_write());

    assert_eq!(VALUE, server_value_received);
    assert_eq!(VALUE, client_tunnel_handle.await.unwrap());

    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_multiple_handshake_v1_successful() {
    let (cert_der, key, cert_hash) = init();
    let server_id = ServerId::try_from("server_id").unwrap();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("ingress").unwrap();

    let server = Server::new_with_certificate(
        server_id.clone(),
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

    server.assign_ingress(mock_ingress.clone_box()).unwrap();

    for _ in 0..3 {
        let client = Client::new();
        let authentication = V1CertifcateHash::new(
            client.clone(),
            auth_key.clone(),
            server.address().unwrap(),
            SERVER_NAME.to_string(),
            cert_hash.clone(),
        );

        let mock_egress =
            MockEgress::new(EgressId::try_from("egress").unwrap(), ingress_id.clone());

        let client_tunnel = authentication.connect(&mock_egress).await.unwrap();

        client_tunnel.close().await;

        sleep(Duration::from_millis(10)).await;

        let server_tunnel = server_tunnel_subscriber.recv().await.unwrap();

        assert_eq!(server_tunnel.id(), client_tunnel.id());
        assert_eq!(server_tunnel.server_id(), client_tunnel.server_id());
        assert_eq!(true, server_tunnel.is_closed());
        assert_eq!(true, client_tunnel.is_closed());

        client.stop().await.unwrap();
    }

    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_handshake_v1_rejected_ingress_id() {
    let (cert_der, key, cert_hash) = init();
    let server_id = ServerId::try_from("server_id").unwrap();

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

    let client = Client::new();

    let authentication = V1CertifcateHash::new(
        client.clone(),
        AuthKey::try_from("valid_auth_key").unwrap(),
        server.address().unwrap(),
        SERVER_NAME.to_string(),
        cert_hash.clone(),
    );

    let mock_egress = MockEgress::new(
        EgressId::try_from("egress").unwrap(),
        IngressId::try_from("ingress").unwrap(),
    );

    if let Err(e) = authentication.connect(&mock_egress).await {
        assert!(matches!(e, Error::RejectedIngressId));
        client.stop().await.unwrap();
        server.stop().await.unwrap();
    } else {
        client.stop().await.unwrap();
        server.stop().await.unwrap();
        panic!("Should not connect.");
    }
}

#[tokio::test]
async fn test_handshake_v1_rejected_auth_key() {
    let (cert_der, key, cert_hash) = init();
    let server_id = ServerId::try_from("server_id").unwrap();

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

    let mock_ingress = MockIngress::new(
        IngressId::try_from("ingress").unwrap(),
        AuthKey::try_from("valid_auth_key").unwrap(),
    );

    server.assign_ingress(mock_ingress.clone_box()).unwrap();

    let client = Client::new();

    let authentication = V1CertifcateHash::new(
        client.clone(),
        AuthKey::try_from("wrong_auth_key").unwrap(),
        server.address().unwrap(),
        SERVER_NAME.to_string(),
        cert_hash.clone(),
    );

    let mock_egress = MockEgress::new(
        EgressId::try_from("egress").unwrap(),
        IngressId::try_from("ingress").unwrap(),
    );

    if let Err(e) = authentication.connect(&mock_egress).await {
        assert!(matches!(e, Error::RejectedAuthKey));
        client.stop().await.unwrap();
        server.stop().await.unwrap();
    } else {
        client.stop().await.unwrap();
        server.stop().await.unwrap();
        panic!("Should not connect.");
    }
}

#[tokio::test]
async fn test_connection_with_wrong_certificate_hash() {
    let (cert_der, key, cert_hash) = init();
    let server_id = ServerId::try_from("server_id").unwrap();
    let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    let ingress_id = IngressId::try_from("ingress").unwrap();

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

    server.assign_ingress(mock_ingress.clone_box()).unwrap();

    let (_, _, wrong_cert_hash, _, _) = generate_self_signed_certificate(SERVER_NAME);

    let client = Client::new();
    let authentication = V1CertifcateHash::new(
        client.clone(),
        auth_key.clone(),
        server.address().unwrap(),
        SERVER_NAME.to_string(),
        wrong_cert_hash.clone(),
    );

    let mock_egress = MockEgress::new(EgressId::try_from("egress").unwrap(), ingress_id.clone());

    let authentication_result = authentication.connect(&mock_egress).await;

    assert!(matches!(authentication_result, Err(Error::TLS(_))));
    assert_eq!(true, authentication_result.err().unwrap().to_string().contains("Unknown error: the cryptographic handshake failed: error 40: unexpected error: certificate hash mismatch"));

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}
