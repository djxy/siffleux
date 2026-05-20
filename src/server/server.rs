use crate::common::error::Error;
use crate::common::message::code::{AUTH_KEY_REJECTED, INGRESS_ID_REJECTED};
use crate::common::message::handshake::{HandshakeV1Request, HandshakeV1Response};
use crate::common::tunnel::Tunnel;
use crate::common::types::{AuthKey, IngressId, TunnelId};
use crate::server::ingress::ingress::Ingress;
use quinn::{Endpoint, Incoming, ServerConfig, VarInt};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tracing::info;

#[derive(Clone)]
pub struct Server {
    inner: Arc<ServerInner>,
}

struct ServerInner {
    auth_key: AuthKey,
    endpoint: Mutex<Option<Endpoint>>,
    ingress_by_id: RwLock<HashMap<IngressId, Arc<dyn Ingress>>>,
    tunnel_id_counter: AtomicU64,
    server_config: ServerConfig,
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
                endpoint: Mutex::new(None),
                ingress_by_id: RwLock::new(HashMap::new()),
                tunnel_id_counter: AtomicU64::new(0),
                server_config,
            }),
        }
    }

    pub fn assign_ingress(&self, ingress: Arc<dyn Ingress>) -> Result<(), Error> {
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

    pub async fn close(&self) -> Result<(), Error> {
        if let Some(endpoint) = self.inner.endpoint.lock()?.take() {
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
                    let handshake = HandshakeV1Request::read(&mut recv).await.unwrap();

                    if handshake.auth_key != self_clone.inner.auth_key {
                        connection.close(AUTH_KEY_REJECTED.code, AUTH_KEY_REJECTED.reason);
                        return;
                    }

                    info!(
                        "Received handshake ingress_id={} name={}.",
                        handshake.ingress_id, handshake.tunnel_name
                    );

                    let Some(ingress) = self_clone
                        .inner
                        .ingress_by_id
                        .read()
                        .unwrap()
                        .get(&handshake.ingress_id)
                        .cloned()
                    else {
                        connection.close(INGRESS_ID_REJECTED.code, INGRESS_ID_REJECTED.reason);
                        return;
                    };

                    let tunnel_id = TunnelId::new(
                        self_clone
                            .inner
                            .tunnel_id_counter
                            .fetch_add(1, Ordering::SeqCst),
                    );

                    info!(
                        "Assign ID={} ingress_id={} name={}.",
                        tunnel_id, handshake.ingress_id, handshake.tunnel_name
                    );

                    HandshakeV1Response::write(&mut send, tunnel_id)
                        .await
                        .unwrap();

                    send.finish().unwrap();

                    let tunnel = Tunnel::new(
                        tunnel_id,
                        handshake.tunnel_name,
                        handshake.ingress_id.clone(),
                        connection,
                    );

                    tunnel.start_hooks();

                    ingress.assign_tunnel(tunnel);
                }
                Err(e) => {
                    connection.close(VarInt::from_u32(1), b"TUNNEL_ID_ERROR");
                    tracing::warn!("Incoming connection didn't received first stream: {e}");
                    return;
                }
            }
        });
    }
}
