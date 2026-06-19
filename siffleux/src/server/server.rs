use crate::Ingress;
use crate::code::{UNKNOWN_ERROR, UNKNOWN_ERROR_SERVER_REASON};
use crate::common::{ByteCounter, IngressId};
use crate::frames::v1::CodecV1;
use crate::server::protocols::v1::{
    handle_server_protocol_v1_auth, handle_server_protocol_v1_command_stream,
};
use crate::{Error, frames};
use quinn::{Endpoint, Incoming, ServerConfig, TransportConfig, VarInt};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::{error, info};

#[derive(Clone)]
pub struct Server {
    pub(in crate::server) inner: Arc<ServerInner>,
}

pub(in crate::server) struct ServerInner {
    endpoint: Mutex<Option<Endpoint>>,
    server_config: ServerConfig,
    pub ingress_by_id: RwLock<HashMap<IngressId, Box<dyn Ingress>>>,
    pub tunnel_id_counter: AtomicU64,
    byte_counter: ByteCounter,
}

impl Server {
    pub fn new_with_certificate(
        certificate_der: CertificateDer<'static>,
        private_key: PrivatePkcs8KeyDer<'static>,
    ) -> Result<Server, Error> {
        let mut tls_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate_der], PrivateKeyDer::from(private_key))?;

        tls_config.alpn_protocols = vec![frames::v1::VERSION.to_vec()];

        let mut transport_config = TransportConfig::default();

        transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
        transport_config.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));

        // TODO: Review those parameters. I just increased them without any meaning
        transport_config.send_window(256 * 1024 * 1024);
        transport_config.receive_window((256 * 1024 * 1024u32).into());
        transport_config.stream_receive_window((64 * 1024 * 1024u32).into());

        transport_config.max_concurrent_bidi_streams(1000u32.into());

        let mut server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)?,
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
                byte_counter: ByteCounter::new(None),
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

    pub fn byte_counter(&self) -> &ByteCounter {
        &self.inner.byte_counter
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

            info!("Server starting listening for tunnels...");

            let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

            // TODO: Review those parameters. I just increased them without any meaning
            socket.set_send_buffer_size(8 * 1024 * 1024)?;
            socket.set_recv_buffer_size(8 * 1024 * 1024)?;
            socket.set_reuse_address(true)?;
            socket.bind(&socket_addr.into())?;

            let std_socket: UdpSocket = socket.into();

            std_socket.set_nonblocking(true)?;

            let endpoint = quinn::Endpoint::new(
                quinn::EndpointConfig::default(),
                Some(self.inner.server_config.clone()),
                std_socket,
                Arc::new(quinn::TokioRuntime),
            )?;

            *endpoint_guard = Some(endpoint.clone());
            endpoint
        };

        info!("Server ready to accept tunnels.");

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
            info!("Closing server...");
            endpoint.close(VarInt::from_u32(0), b"done");
            endpoint.wait_idle().await;
            info!("Server closed.");

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
                    error!("Incoming QUIC connection failed: {e}");
                    return;
                }
            };

            match connection.accept_bi().await {
                Ok((send, recv)) => {
                    let mut write_framed = FramedWrite::new(send, CodecV1);
                    let mut read_framed = FramedRead::new(recv, CodecV1);

                    let (ingress, tunnel) = match handle_server_protocol_v1_auth(
                        self_clone.clone(),
                        connection,
                        &mut write_framed,
                        &mut read_framed,
                    )
                    .await
                    {
                        Ok((ingress, tunnel)) => (ingress, tunnel),
                        Err(e) => {
                            error!("Error while authenticating tunnel: {e}");
                            return;
                        }
                    };

                    if let Err(e) = ingress.assign_tunnel(tunnel.clone()) {
                        error!("Error while authenticating tunnel: {e}");
                    }

                    if let Err(e) = handle_server_protocol_v1_command_stream(
                        tunnel.clone(),
                        write_framed,
                        read_framed,
                    )
                    .await
                    {
                        tunnel
                            .connection()
                            .close(UNKNOWN_ERROR, UNKNOWN_ERROR_SERVER_REASON);

                        error!(tunnel_id = %&tunnel.id(), "Tunnel closed. Error with command stream: {e}");
                    } else {
                        info!(tunnel_id = %&tunnel.id(), "Tunnel closed.");
                    }
                }
                Err(e) => {
                    connection.close(UNKNOWN_ERROR, UNKNOWN_ERROR_SERVER_REASON);

                    error!("QUIC connection failed to accept the first stream: {e}");

                    return;
                }
            }
        });
    }
}
