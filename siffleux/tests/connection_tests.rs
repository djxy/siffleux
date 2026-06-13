// use async_trait::async_trait;
// use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
// use siffleux::codes::CLOSED;
// use siffleux::ingress::{Ingress, IngressClone};
// use siffleux::{
//     AuthKey, Error, HashedAuthKey, IngressId, Server, Tunnel, TunnelId, TunnelName,
//     generate_self_signed_certificate,
// };
// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
// use std::sync::{Arc, Mutex, OnceLock};
// use std::time::Duration;
// use tokio::time::sleep;

// static INIT: OnceLock<(
//     CertificateDer<'static>,
//     PrivatePkcs8KeyDer<'static>,
//     Vec<u8>,
// )> = OnceLock::new();

// static SERVER_NAME: &'static str = "localhost";

// #[derive(Clone)]
// struct MockIngress {
//     inner: Arc<MockIngressInner>,
// }

// struct MockIngressInner {
//     id: IngressId,
//     hashed_auth_key: HashedAuthKey,
//     tunnels: Mutex<Vec<Tunnel>>,
// }

// impl MockIngress {
//     fn new(id: IngressId, hashed_auth_key: HashedAuthKey) -> Self {
//         Self {
//             inner: Arc::new(MockIngressInner {
//                 id,
//                 hashed_auth_key,
//                 tunnels: Mutex::new(vec![]),
//             }),
//         }
//     }
// }

// #[async_trait]
// impl Ingress for MockIngress {
//     fn id(&self) -> &IngressId {
//         &self.inner.id
//     }

//     fn hashed_auth_key(&self) -> &HashedAuthKey {
//         &self.inner.hashed_auth_key
//     }

//     fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
//         self.inner.tunnels.lock().unwrap().push(tunnel);

//         Ok(())
//     }

//     async fn start(&self) -> Result<(), Error> {
//         Ok(())
//     }

//     async fn stop(&self) -> Result<(), Error> {
//         Ok(())
//     }
// }

// fn init() -> &'static (
//     CertificateDer<'static>,
//     PrivatePkcs8KeyDer<'static>,
//     Vec<u8>,
// ) {
//     INIT.get_or_init(|| {
//         let _ = tracing_subscriber::fmt().with_test_writer().try_init();

//         rustls::crypto::ring::default_provider()
//             .install_default()
//             .unwrap();

//         generate_self_signed_certificate(SERVER_NAME)
//     })
// }

// #[tokio::test]
// async fn test_detect_tunnel_closed() {
//     let (cert_der, key, _cert_hash) = init();
//     let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
//     let ingress_id = IngressId::try_from("ingress").unwrap();

//     let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

//     server
//         .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
//         .await
//         .unwrap();

//     let mock_ingress = MockIngress::new(ingress_id.clone(), auth_key.hash());

//     server.assign_ingress(mock_ingress.clone_box()).unwrap();

//     let tunnel = Tunnel::connect_to_server_with_certificates(
//         auth_key,
//         ingress_id.clone(),
//         TunnelName::try_from("").unwrap(),
//         server.address().unwrap(),
//         SERVER_NAME.to_string(),
//         vec![cert_der.clone()],
//     )
//     .await
//     .unwrap();

//     let tunnel_close_1 = tunnel.clone();
//     let tunnel_close_2 = tunnel.clone();

//     let close_handle_1 = tokio::spawn(async move {
//         tunnel_close_1.closed().await;
//     });

//     let close_handle_2 = tokio::spawn(async move {
//         tunnel_close_2.closed().await;
//     });

//     server.stop().await.unwrap();

//     close_handle_1.await.unwrap();
//     close_handle_2.await.unwrap();
// }

// #[tokio::test]
// async fn test_send_data_over_stream() {
//     let (cert_der, key, _cert_hash) = init();
//     let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
//     let ingress_id = IngressId::try_from("ingress").unwrap();

//     let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

//     server
//         .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
//         .await
//         .unwrap();

//     let mock_ingress = MockIngress::new(ingress_id.clone(), auth_key.hash());

//     server.assign_ingress(mock_ingress.clone_box()).unwrap();

//     let tunnel = Tunnel::connect_to_server_with_certificates(
//         auth_key,
//         ingress_id.clone(),
//         TunnelName::try_from("").unwrap(),
//         server.address().unwrap(),
//         SERVER_NAME.to_string(),
//         vec![cert_der.clone()],
//     )
//     .await
//     .unwrap();

//     let server_tunnel = mock_ingress.inner.tunnels.lock().unwrap().pop().unwrap();

//     let value: u64 = 6329282199514132237;

//     let tunnel_receive = tunnel.clone();

//     let tunnel_handle = tokio::spawn(async move {
//         let mut buffer = [0u8; 16];
//         let (mut tunnel_read_channel, _) = tunnel_receive.accept_stream().await.unwrap();
//         let size_opt = tunnel_read_channel.read(&mut buffer[..8]).await.unwrap();

//         let value_received = u64::from_be_bytes(buffer[..size_opt.unwrap()].try_into().unwrap());

//         assert_eq!(8, tunnel_receive.bytes_received());

//         value_received
//     });

//     let (_, mut server_write_channel) = server_tunnel.create_stream().await.unwrap();

//     server_write_channel
//         .write(&mut value.to_be_bytes())
//         .await
//         .unwrap();

//     assert_eq!(8, server_tunnel.bytes_sent());
//     assert_eq!(value, tunnel_handle.await.unwrap());
//     assert_eq!(8, tunnel.bytes_received());

//     server.stop().await.unwrap();
// }

// #[tokio::test]
// async fn test_multiple_handshake_v1_successful() {
//     let (cert_der, key, _cert_hash) = init();
//     let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
//     let ingress_id = IngressId::try_from("ingress").unwrap();

//     let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

//     server
//         .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
//         .await
//         .unwrap();

//     let mock_ingress = MockIngress::new(ingress_id.clone(), auth_key.hash());

//     server.assign_ingress(mock_ingress.clone_box()).unwrap();

//     for i in 0..3 {
//         let tunnel_name = TunnelName::try_from(format!("name-{i}")).unwrap();

//         let tunnel = Tunnel::connect_to_server_with_certificates(
//             auth_key.clone(),
//             ingress_id.clone(),
//             tunnel_name.clone(),
//             SocketAddr::new(
//                 IpAddr::V4(Ipv4Addr::LOCALHOST),
//                 server.address().unwrap().port(),
//             ),
//             SERVER_NAME.to_string(),
//             vec![cert_der.clone()],
//         )
//         .await
//         .unwrap();

//         tunnel.close(&CLOSED);

//         sleep(Duration::from_millis(10)).await;

//         let server_tunnel = mock_ingress.inner.tunnels.lock().unwrap().pop().unwrap();

//         assert_eq!(TunnelId::new(i), server_tunnel.id());
//         assert_eq!(ingress_id, server_tunnel.ingress_id().clone());
//         assert_eq!(tunnel_name, server_tunnel.name().clone());
//         assert_eq!(true, server_tunnel.is_closed());
//     }

//     server.stop().await.unwrap();
// }

// #[tokio::test]
// async fn test_handshake_v1_rejected_ingress_id() {
//     let (cert_der, key, _cert_hash) = init();

//     let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

//     server
//         .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
//         .await
//         .unwrap();

//     if let Err(e) = Tunnel::connect_to_server_with_certificates(
//         AuthKey::try_from("valid_auth_key").unwrap(),
//         IngressId::try_from("iii").unwrap(),
//         TunnelName::try_from("ttt").unwrap(),
//         server.address().unwrap(),
//         SERVER_NAME.to_string(),
//         vec![cert_der.clone()],
//     )
//     .await
//     {
//         assert!(matches!(e, Error::IngressIdRejected));
//         server.stop().await.unwrap();
//     } else {
//         server.stop().await.unwrap();
//         panic!("Should not connect.");
//     }
// }

// #[tokio::test]
// async fn test_handshake_v1_rejected_auth_key() {
//     let (cert_der, key, _cert_hash) = init();
//     let ingress_id = IngressId::try_from("iii").unwrap();

//     let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

//     server
//         .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
//         .await
//         .unwrap();

//     let mock_ingress = MockIngress::new(
//         ingress_id.clone(),
//         AuthKey::try_from("valid_auth_key").unwrap().hash(),
//     );

//     server.assign_ingress(mock_ingress.clone_box()).unwrap();

//     if let Err(e) = Tunnel::connect_to_server_with_certificates(
//         AuthKey::try_from("wrong_auth_key").unwrap(),
//         ingress_id.clone(),
//         TunnelName::try_from("ttt").unwrap(),
//         server.address().unwrap(),
//         SERVER_NAME.to_string(),
//         vec![cert_der.clone()],
//     )
//     .await
//     {
//         assert!(matches!(e, Error::AuthKeyRejected));
//         server.stop().await.unwrap();
//     } else {
//         server.stop().await.unwrap();
//         panic!("Should not authenticate.");
//     }
// }

// #[tokio::test]
// async fn test_connection_with_certificate_hash() {
//     let (cert_der, key, cert_hash) = init();
//     let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
//     let ingress_id = IngressId::try_from("ingress").unwrap();

//     let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

//     server
//         .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
//         .await
//         .unwrap();

//     let mock_ingress = MockIngress::new(ingress_id.clone(), auth_key.hash());

//     server.assign_ingress(mock_ingress.clone_box()).unwrap();

//     let _ = Tunnel::connect_to_server_with_certificate_hash(
//         auth_key,
//         ingress_id.clone(),
//         TunnelName::try_from("ttt").unwrap(),
//         SocketAddr::new(
//             IpAddr::V4(Ipv4Addr::LOCALHOST),
//             server.address().unwrap().port(),
//         ),
//         SERVER_NAME.to_string(),
//         cert_hash.clone(),
//     )
//     .await
//     .unwrap();

//     server.stop().await.unwrap();
// }

// #[tokio::test]
// async fn test_connection_with_wrong_certificate_hash() {
//     let (cert_der, key, _) = init();
//     let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
//     let ingress_id = IngressId::try_from("ingress").unwrap();

//     let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

//     server
//         .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
//         .await
//         .unwrap();

//     let mock_ingress = MockIngress::new(ingress_id.clone(), auth_key.hash());

//     server.assign_ingress(mock_ingress.clone_box()).unwrap();

//     let (_, _, wrong_cert_hash) = generate_self_signed_certificate(SERVER_NAME);

//     let result = Tunnel::connect_to_server_with_certificate_hash(
//         auth_key,
//         ingress_id.clone(),
//         TunnelName::try_from("ttt").unwrap(),
//         SocketAddr::new(
//             IpAddr::V4(Ipv4Addr::LOCALHOST),
//             server.address().unwrap().port(),
//         ),
//         SERVER_NAME.to_string(),
//         wrong_cert_hash.clone(),
//     )
//     .await;

//     assert!(matches!(result, Err(Error::TLS(_))));
//     assert_eq!(true, result.err().unwrap().to_string().contains("Unknown error: the cryptographic handshake failed: error 40: unexpected error: certificate hash mismatch"));

//     server.stop().await.unwrap();
// }
