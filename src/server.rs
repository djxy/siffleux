use quinn::{Connection, Endpoint, Incoming, RecvStream, SendStream, ServerConfig, VarInt};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
pub struct TunnelServer {
    inner: Arc<TunnelServerInner>,
}

pub struct TunnelServerInner {
    server_config: ServerConfig,
    endpoint: RwLock<Option<Endpoint>>,
    tunnels_by_ingress_id: RwLock<HashMap<Uuid, HashMap<Uuid, Connection>>>,
}

impl Deref for TunnelServer {
    type Target = TunnelServerInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl TunnelServer {
    pub fn new_with_self_signed_certificate(
        certificate_der: CertificateDer<'static>,
        private_key: PrivatePkcs8KeyDer<'static>,
    ) -> anyhow::Result<TunnelServer> {
        let crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate_der], PrivateKeyDer::from(private_key))?;

        let server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(crypto)?,
        ));

        Ok(TunnelServer::new(server_config))
    }

    pub fn new(server_config: ServerConfig) -> TunnelServer {
        TunnelServer {
            inner: Arc::new(TunnelServerInner {
                server_config,
                endpoint: RwLock::new(None),
                tunnels_by_ingress_id: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub async fn listen(&self, socket_addr: SocketAddr) -> anyhow::Result<()> {
        let endpoint = {
            let mut endpoint_guard = self.endpoint.write().await;

            if endpoint_guard.is_some() {
                return Err(anyhow::anyhow!("Already listening."));
            }

            let endpoint = Endpoint::server(self.server_config.clone(), socket_addr)?;

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
                    let mut buffer = [0u8; 16 + 16];
                    recv.read_exact(&mut buffer).await.unwrap();
                    send.finish();

                    let tunnel_id = Uuid::from_slice(&buffer[..16]).unwrap();
                    let ingress_id = Uuid::from_slice(&buffer[16..]).unwrap();

                    info!("Received tunnel_id={tunnel_id} ingress_id={ingress_id}");

                    self_clone.handle_connection_close(connection.clone(), ingress_id, tunnel_id);

                    {
                        let mut tunnels_by_ingress_id =
                            self_clone.tunnels_by_ingress_id.write().await;

                        tunnels_by_ingress_id
                            .entry(ingress_id)
                            .or_insert_with(HashMap::new)
                            .insert(tunnel_id, connection.clone());
                    }

                    sleep(Duration::from_millis(10)).await;
                }
                Err(e) => {
                    connection.close(VarInt::from_u32(1), b"TUNNEL_ID_ERROR");
                    tracing::warn!("Incoming connection didn't received first stream: {e}");
                    return;
                }
            }
        });
    }

    fn handle_connection_close(&self, connection: Connection, ingress_id: Uuid, tunnel_id: Uuid) {
        let self_clone = self.clone();
        tokio::spawn(async move {
            info!(
                "tunnel_id={tunnel_id} closed: {:?}",
                connection.closed().await
            );

            if let Some(tunnels) = self_clone
                .tunnels_by_ingress_id
                .write()
                .await
                .get_mut(&ingress_id)
            {
                tunnels.remove(&tunnel_id);
                info!("tunnels={}", tunnels.len());
            }
        });
    }
}
