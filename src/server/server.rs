use crate::error::Error;
use crate::message::code::WRONG_AUTH_KEY;
use crate::message::handshake::{HandshakeV1Request, HandshakeV1Response};
use crate::server::ingress::ingress::Ingress;
use crate::server::server_tunnel::ServerTunnel;
use crate::types::{AuthKey, IngressId, TunnelId};
use quinn::{Endpoint, Incoming, ServerConfig, VarInt};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use tokio::sync::RwLock;
use tracing::info;

#[derive(Clone)]
pub struct Server {
    inner: Arc<ServerInner>,
}

pub struct ServerInner {
    port: AtomicU16,
    auth_key: AuthKey,
    endpoint: RwLock<Option<Endpoint>>,
    ingress_by_id: RwLock<HashMap<IngressId, Arc<dyn Ingress>>>,
    id_counter: AtomicU64,
    server_config: ServerConfig,
}

impl Deref for Server {
    type Target = ServerInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Server {
    pub fn new_with_self_signed_certificate(
        auth_key: AuthKey,
        certificate_der: CertificateDer<'static>,
        private_key: PrivatePkcs8KeyDer<'static>,
    ) -> Result<Server, Error> {
        let crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate_der], PrivateKeyDer::from(private_key))?;

        let server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(crypto)?,
        ));

        Ok(Server::new(auth_key, server_config))
    }

    fn new(auth_key: AuthKey, server_config: ServerConfig) -> Server {
        Server {
            inner: Arc::new(ServerInner {
                auth_key,
                port: AtomicU16::new(0),
                endpoint: RwLock::new(None),
                ingress_by_id: RwLock::new(HashMap::new()),
                id_counter: AtomicU64::new(0),
                server_config,
            }),
        }
    }

    pub fn port(&self) -> u16 {
        self.port.load(Ordering::SeqCst)
    }

    pub async fn listen(&self, socket_addr: SocketAddr) -> Result<(), Error> {
        let endpoint = {
            let mut endpoint_guard = self.endpoint.write().await;

            if endpoint_guard.is_some() {
                return Err(Error::ServerAlreadyListening);
            }

            let endpoint = Endpoint::server(self.server_config.clone(), socket_addr)?;

            self.port
                .store(endpoint.local_addr()?.port(), Ordering::SeqCst);

            *endpoint_guard = Some(endpoint.clone());
            endpoint
        };

        let self_clone = self.clone();

        tokio::spawn(async move {
            while let Some(incoming_connection) = endpoint.accept().await {
                self_clone.handle_connection(incoming_connection);
            }
        });

        Ok(())
    }

    pub async fn close(&self) {
        if let Some(endpoint) = self.endpoint.write().await.take() {
            endpoint.close(VarInt::from_u32(0), b"done");
        }
    }

    fn handle_connection(&self, incoming_connection: Incoming) {
        let self_clone = self.clone();

        tokio::spawn(async move {
            let connection = match incoming_connection.await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::warn!("Incoming connection failed: {e}");
                    return;
                }
            };

            match connection.accept_bi().await {
                Ok((mut send, mut recv)) => {
                    let handshake = HandshakeV1Request::read(&mut recv).await.unwrap();

                    if handshake.auth_key != self_clone.auth_key {
                        connection.close(WRONG_AUTH_KEY.code, WRONG_AUTH_KEY.reason);
                        return;
                    }

                    info!(
                        "Received handshake ingress_id={} name={}.",
                        handshake.ingress_id, handshake.tunnel_name
                    );

                    let tunnel_id =
                        TunnelId::new(self_clone.id_counter.fetch_add(1, Ordering::SeqCst));

                    info!(
                        "Assign ID={} ingress_id={} name={}.",
                        tunnel_id, handshake.ingress_id, handshake.tunnel_name
                    );

                    HandshakeV1Response::write(&mut send, tunnel_id)
                        .await
                        .unwrap();

                    send.finish().unwrap();

                    let tunnel_connection = Arc::new(ServerTunnel::new(
                        tunnel_id,
                        handshake.tunnel_name,
                        handshake.ingress_id,
                        connection,
                    ));

                    self_clone.handle_connection_close(tunnel_connection.clone());
                }
                Err(e) => {
                    connection.close(VarInt::from_u32(1), b"TUNNEL_ID_ERROR");
                    tracing::warn!("Incoming connection didn't received first stream: {e}");
                    return;
                }
            }
        });
    }

    fn handle_connection_close(&self, tunnel_connection: Arc<ServerTunnel>) {
        tokio::spawn(async move {
            info!(
                "tunnel_id={} closed: {:?}",
                tunnel_connection.id(),
                tunnel_connection.connection().closed().await
            );
        });
    }
}
