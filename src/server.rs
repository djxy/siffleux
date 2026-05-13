use quinn::{ConnectionError, Endpoint, Incoming, RecvStream, SendStream, ServerConfig, VarInt};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
pub struct TunnelServer {
    inner: Arc<TunnelServerInner>,
}

pub struct TunnelServerInner {
    server_config: ServerConfig,
    endpoint: RwLock<Option<Endpoint>>,
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
        tokio::spawn(async move {
            let connection = match incoming_connection.await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::warn!("Incoming connection failed: {e}");
                    return;
                }
            };

            match connection.accept_bi().await {
                Ok((send, mut recv)) => {
                    let mut tunnel_id_buffer = [0u8; 16];
                    recv.read_exact(&mut tunnel_id_buffer).await.unwrap();
                    let tunnel_id = Uuid::from_slice(&tunnel_id_buffer).unwrap();

                    info!("Received tunnel_id {tunnel_id}");
                    connection.close(VarInt::from_u32(0), b"done");
                }
                Err(e) => {
                    tracing::warn!("Incoming connection didn't received first stream: {e}");
                }
            }
        });
    }
}
