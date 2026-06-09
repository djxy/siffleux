use crate::codes::{AUTH_KEY_REJECTED, INGRESS_ID_REJECTED};
use crate::ingress::Ingress;
use crate::server::protocols::v1::handle_protocol_v1_auth;
use crate::{Error, IngressId, Tunnel, TunnelId, frames};
use quinn::{Endpoint, Incoming, ServerConfig, TransportConfig, VarInt};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tracing::{info, warn};

#[derive(Clone)]
pub struct Server {
    pub(super) inner: Arc<ServerInner>,
}

struct ServerInner {
    endpoint: Mutex<Option<Endpoint>>,
    server_config: ServerConfig,
    pub(super) ingress_by_id: RwLock<HashMap<IngressId, Box<dyn Ingress>>>,
    pub(super) tunnel_id_counter: AtomicU64,
}

impl Server {
    pub fn new_with_certificate(
        certificate_der: CertificateDer<'static>,
        private_key: PrivatePkcs8KeyDer<'static>,
    ) -> Result<Server, Error> {
        let mut crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate_der], PrivateKeyDer::from(private_key))?;

        crypto.alpn_protocols = vec![frames::v1::VERSION.to_vec()];

        let mut transport_config = TransportConfig::default();

        transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
        transport_config.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));

        let mut server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(crypto)?,
        ));

        server_config.transport_config(Arc::new(transport_config));

        Ok(Server::new(server_config))
    }

    fn new(server_config: ServerConfig) -> Server {
        Server {
            inner: Arc::new(ServerInner {
                endpoint: Mutex::new(None),
                ingress_by_id: RwLock::new(HashMap::new()),
                tunnel_id_counter: AtomicU64::new(0),
                server_config,
            }),
        }
    }

    pub fn assign_ingress(&self, ingress: Box<dyn Ingress>) -> Result<(), Error> {
        let mut ingress_by_id = self.inner.ingress_by_id.write()?;

        if ingress_by_id.contains_key(&ingress.id()) {
            return Err(Error::IngressIDAlreadyAssigned(ingress.id().clone()));
        }

        ingress_by_id.insert(ingress.id().clone(), ingress);

        Ok(())
    }

    pub fn address(&self) -> Result<SocketAddr, Error> {
        self.inner
            .endpoint
            .lock()?
            .as_ref()
            .ok_or(Error::ServerNotListening)?
            .local_addr()
            .map_err(Error::from)
    }

    pub async fn listen(&self, socket_addr: SocketAddr) -> Result<(), Error> {
        let endpoint = {
            let mut endpoint_guard = self.inner.endpoint.lock()?;

            if endpoint_guard.is_some() {
                return Err(Error::ServerAlreadyListening);
            }

            let endpoint = Endpoint::server(self.inner.server_config.clone(), socket_addr)?;

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

    pub async fn stop(&self) -> Result<(), Error> {
        if let Some(endpoint) = self.inner.endpoint.lock()?.take() {
            info!("Closing server");
            endpoint.close(VarInt::from_u32(0), b"done");

            Ok(())
        } else {
            Err(Error::ServerNotListening)
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
                    if let Err(e) =
                        handle_protocol_v1_auth(self_clone.clone(), connection.clone(), send, recv)
                            .await
                    {
                        warn!("Auth failed: {e}");

                        return;
                    }
                    let handshake = authentication::v1::Request::read(&mut recv).await.unwrap();

                    let Some(ingress) = self_clone
                        .inner
                        .ingress_by_id
                        .read()
                        .unwrap()
                        .get(&handshake.ingress_id)
                        .cloned()
                    else {
                        warn!(
                            "Refused handshake from tunnel_name={}. ingress_id={} doesn't exist.",
                            handshake.tunnel_name, handshake.ingress_id,
                        );

                        connection.close(INGRESS_ID_REJECTED.code, INGRESS_ID_REJECTED.reason);
                        return;
                    };

                    if !ingress.hashed_auth_key().verify(&handshake.auth_key) {
                        warn!(
                            "Refused handshake from tunnel_name={}. Rejected auth_key.",
                            handshake.tunnel_name
                        );

                        connection.close(AUTH_KEY_REJECTED.code, AUTH_KEY_REJECTED.reason);
                        return;
                    }

                    let tunnel_id = TunnelId::new(
                        self_clone
                            .inner
                            .tunnel_id_counter
                            .fetch_add(1, Ordering::SeqCst),
                    );

                    info!(
                        "Assigned tunnel_id={} to tunnel_name={} on ingress_id={}",
                        tunnel_id, handshake.tunnel_name, handshake.ingress_id
                    );

                    authentication::v1::Response::write(&mut send, tunnel_id)
                        .await
                        .unwrap();

                    send.finish().unwrap();

                    let tunnel = Tunnel::new(
                        tunnel_id,
                        handshake.tunnel_name,
                        handshake.ingress_id.clone(),
                        connection,
                    );

                    let _ = ingress.assign_tunnel(tunnel);
                }
                Err(e) => {
                    connection.close(VarInt::from_u32(1), b"TUNNEL_ID_ERROR");
                    warn!("Incoming connection didn't received first stream: {e}");
                    return;
                }
            }
        });
    }
}
