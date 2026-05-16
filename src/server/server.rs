use crate::message::code::WRONG_AUTH_KEY;
use crate::message::handshake::{HandshakeV1Request, HandshakeV1Response};
use crate::server::tunnel_connection::TunnelConnection;
use quinn::{Endpoint, Incoming, ServerConfig, VarInt};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
pub struct Server {
    inner: Arc<ServerInner>,
}

pub struct ServerInner {
    port: AtomicU16,
    auth_key: String,
    server_config: ServerConfig,
    endpoint: RwLock<Option<Endpoint>>,
}

impl Deref for Server {
    type Target = ServerInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Server {
    pub fn new_with_self_signed_certificate(
        auth_key: &str,
        certificate_der: CertificateDer<'static>,
        private_key: PrivatePkcs8KeyDer<'static>,
    ) -> anyhow::Result<Server> {
        let crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate_der], PrivateKeyDer::from(private_key))?;

        let server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(crypto)?,
        ));

        Ok(Server::new(auth_key, server_config))
    }

    pub fn new(auth_key: &str, server_config: ServerConfig) -> Server {
        Server {
            inner: Arc::new(ServerInner {
                auth_key: auth_key.to_string(),
                server_config,
                port: AtomicU16::new(0),
                endpoint: RwLock::new(None),
            }),
        }
    }

    pub fn port(&self) -> u16 {
        self.port.load(Ordering::SeqCst)
    }

    pub async fn listen(&self, socket_addr: SocketAddr) -> anyhow::Result<()> {
        let endpoint = {
            let mut endpoint_guard = self.endpoint.write().await;

            if endpoint_guard.is_some() {
                return Err(anyhow::anyhow!("Already listening."));
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
                        "Received handshake auth_key={} ingress_id={} name={}.",
                        handshake.auth_key, handshake.ingress_id, handshake.name
                    );

                    let id = Uuid::new_v4();

                    info!(
                        "Assign ID={} auth_key={} ingress_id={} name={}.",
                        id, handshake.auth_key, handshake.ingress_id, handshake.name
                    );

                    HandshakeV1Response::write(&mut send, id).await.unwrap();

                    send.finish().unwrap();

                    let tunnel_connection = Arc::new(TunnelConnection::new(
                        id,
                        handshake.name,
                        handshake.ingress_id,
                        connection,
                    ));

                    self_clone.handle_connection_close(tunnel_connection.clone());

                    // {
                    //     let mut tunnels_by_ingress_id =
                    //         self_clone.tunnels_by_ingress_id.write().await;
                    //
                    //     tunnels_by_ingress_id
                    //         .entry(ingress_id)
                    //         .or_insert_with(HashMap::new)
                    //         .insert(id, tunnel_connection);
                    // }
                }
                Err(e) => {
                    connection.close(VarInt::from_u32(1), b"TUNNEL_ID_ERROR");
                    tracing::warn!("Incoming connection didn't received first stream: {e}");
                    return;
                }
            }
        });
    }

    fn handle_connection_close(&self, tunnel_connection: Arc<TunnelConnection>) {
        tokio::spawn(async move {
            info!(
                "tunnel_id={} closed: {:?}",
                tunnel_connection.id(),
                tunnel_connection.connection().closed().await
            );
        });
    }
}
