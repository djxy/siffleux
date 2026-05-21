use async_trait::async_trait;
use kipawa::code::CLOSED;
use kipawa::ingress::Ingress;
use kipawa::{AuthKey, Client, Error, IngressId, Server, Tunnel, TunnelId, TunnelName};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::time::sleep;

static CRYPTO: OnceLock<(CertificateDer<'static>, PrivatePkcs8KeyDer<'static>)> = OnceLock::new();

static SERVER_NAME: &'static str = "localhost";

struct MockIngress {
    id: IngressId,
    tunnels: Mutex<Vec<Tunnel>>,
}

impl MockIngress {
    fn new(id: IngressId) -> Self {
        Self {
            id,
            tunnels: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl Ingress for MockIngress {
    fn id(&self) -> &IngressId {
        &self.id
    }

    fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
        self.tunnels.lock().unwrap().push(tunnel);

        Ok(())
    }

    async fn start(&self, _server: &Server) -> Result<(), Error> {
        Ok(())
    }

    async fn stop(&self, _server: &Server) -> Result<(), Error> {
        Ok(())
    }
}

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
async fn test_detect_tunnel_closed() {
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

    let mock_ingress = Arc::new(MockIngress::new(ingress_id.clone()));

    server.assign_ingress(mock_ingress.clone()).unwrap();

    let client = Client::connect_with_certificates(
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

    let client_tunnel_close_1 = client.tunnel().clone();
    let client_tunnel_close_2 = client.tunnel().clone();

    let close_handle_1 = tokio::spawn(async move {
        client_tunnel_close_1.closed().await;
    });

    let close_handle_2 = tokio::spawn(async move {
        client_tunnel_close_2.closed().await;
    });

    server.close().await.unwrap();

    close_handle_1.await.unwrap();
    close_handle_2.await.unwrap();
}

#[tokio::test]
async fn test_send_data_over_stream() {
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

    let mock_ingress = Arc::new(MockIngress::new(ingress_id.clone()));

    server.assign_ingress(mock_ingress.clone()).unwrap();

    let client = Client::connect_with_certificates(
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

    let server_tunnel = mock_ingress.tunnels.lock().unwrap().pop().unwrap();

    let value: u64 = 6329282199514132237;

    let client_tunnel_receive = client.tunnel().clone();

    let client_handle = tokio::spawn(async move {
        let mut buffer = [0u8; 16];
        let (mut client_read_stream, _) = client_tunnel_receive.accept_stream().await.unwrap();
        let size_opt = client_read_stream.read(&mut buffer[..8]).await.unwrap();

        let value_received = u64::from_be_bytes(buffer[..size_opt.unwrap()].try_into().unwrap());

        assert_eq!(8, client_tunnel_receive.bytes_received());

        value_received
    });

    let (_, mut server_send_stream) = server_tunnel.create_stream().await.unwrap();

    server_send_stream
        .write(&mut value.to_be_bytes())
        .await
        .unwrap();

    assert_eq!(8, server_tunnel.bytes_sent());
    assert_eq!(value, client_handle.await.unwrap());

    server.close().await.unwrap();
}

#[tokio::test]
async fn test_multiple_handshake_v1_successful() {
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

    let mock_ingress = Arc::new(MockIngress::new(ingress_id.clone()));

    server.assign_ingress(mock_ingress.clone()).unwrap();

    for i in 0..3 {
        let tunnel_name = TunnelName::try_from(format!("name-{i}")).unwrap();

        let client = Client::connect_with_certificates(
            auth_key.clone(),
            ingress_id.clone(),
            tunnel_name.clone(),
            SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                server.address().unwrap().port(),
            ),
            SERVER_NAME.to_string(),
            vec![cert_der.clone()],
        )
        .await
        .unwrap();

        client.tunnel().close(&CLOSED);

        sleep(Duration::from_millis(10)).await;

        let server_tunnel = mock_ingress.tunnels.lock().unwrap().pop().unwrap();

        assert_eq!(TunnelId::new(i), server_tunnel.id());
        assert_eq!(ingress_id, server_tunnel.ingress_id().clone());
        assert_eq!(tunnel_name, server_tunnel.name().clone());
        assert_eq!(true, server_tunnel.is_closed());
    }

    server.close().await.unwrap();
}

#[tokio::test]
async fn test_handshake_v1_rejected_ingress_id() {
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

    if let Err(e) = Client::connect_with_certificates(
        AuthKey::try_from("valid_auth_key").unwrap(),
        IngressId::try_from("").unwrap(),
        TunnelName::try_from("").unwrap(),
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            server.address().unwrap().port(),
        ),
        SERVER_NAME.to_string(),
        vec![cert_der.clone()],
    )
    .await
    {
        matches!(e, Error::IngressIdRejected);
        server.close().await.unwrap();
    } else {
        server.close().await.unwrap();
        panic!("Should not connect.");
    }
}

#[tokio::test]
async fn test_handshake_v1_rejected_auth_key() {
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

    if let Err(e) = Client::connect_with_certificates(
        AuthKey::try_from("wrong_auth_key").unwrap(),
        IngressId::try_from("").unwrap(),
        TunnelName::try_from("").unwrap(),
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            server.address().unwrap().port(),
        ),
        SERVER_NAME.to_string(),
        vec![cert_der.clone()],
    )
    .await
    {
        matches!(e, Error::AuthKeyRejected);
        server.close().await.unwrap();
    } else {
        server.close().await.unwrap();
        panic!("Should not authenticate.");
    }
}
