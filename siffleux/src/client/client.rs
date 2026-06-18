use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::Arc,
};

use quinn::{ClientConfig, Endpoint, TransportConfig, crypto::rustls::QuicClientConfig};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::lookup_host;
use tracing::info;

use crate::{
    Error, Tunnel,
    client::{
        certificate_verifier::CertificateHashVerifier,
        protocols::v1::handle_client_protocol_v1_auth,
    },
    common::{AuthKey, ByteCounter, IngressId, TunnelName},
    frames,
};

#[derive(Debug, Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

#[derive(Debug)]
struct ClientInner {
    byte_counter: ByteCounter,
}

impl Client {
    pub fn new() -> Self {
        Client {
            inner: Arc::new(ClientInner {
                byte_counter: ByteCounter::new(None),
            }),
        }
    }

    pub async fn connect_tunnel_with_certificate_hash(
        &self,
        auth_key: AuthKey,
        ingress_id: IngressId,
        name: TunnelName,
        server_address: SocketAddr,
        server_name: String,
        certificate_hash: Vec<u8>,
    ) -> Result<(Tunnel, Endpoint), Error> {
        let mut tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(CertificateHashVerifier::new(
                certificate_hash,
            )))
            .with_no_client_auth();

        tls_config.alpn_protocols = vec![frames::v1::VERSION.to_vec()];

        info!(server = %server_address, ingress_id = %ingress_id.clone(), "Connecting to server...");

        let mut transport_config = TransportConfig::default();

        // TODO: Review those parameters. I just increased them without any meaning
        transport_config.send_window(256 * 1024 * 1024);
        transport_config.receive_window((256 * 1024 * 1024u32).into());
        transport_config.stream_receive_window((64 * 1024 * 1024u32).into());

        transport_config.max_concurrent_bidi_streams(1000u32.into());

        let mut client_config =
            ClientConfig::new(Arc::new(QuicClientConfig::try_from(tls_config)?));

        client_config.transport_config(Arc::new(transport_config));

        // TODO: Change how the lookup_host is passed. Currently it is for the docker compose
        info!("Lookup server");
        let mut addresses = lookup_host("server:8765").await?;
        info!("Lookup serve done");

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

        // TODO: Review those parameters. I just increased them without any meaning
        socket.set_send_buffer_size(8 * 1024 * 1024)?;
        socket.set_recv_buffer_size(8 * 1024 * 1024)?;
        socket.set_reuse_address(true)?;
        socket.bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0).into())?;

        if let Ok(actual_send) = socket.send_buffer_size() {
            info!("Kernel Send Buffer Size: {} bytes", actual_send);
        }

        if let Ok(actual_recv) = socket.recv_buffer_size() {
            info!("Kernel Receive Buffer Size: {} bytes", actual_recv);
        }

        let std_socket: UdpSocket = socket.into();

        std_socket.set_nonblocking(true)?;

        // let endpoint = Endpoint::client(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))?;

        let endpoint = quinn::Endpoint::new(
            quinn::EndpointConfig::default(),
            None,
            std_socket,
            Arc::new(quinn::TokioRuntime),
        )?;

        let tunnel = handle_client_protocol_v1_auth(
            endpoint
                .connect_with(client_config, addresses.next().unwrap(), &server_name)?
                .await?,
            auth_key,
            ingress_id.clone(),
            name,
            &self.inner.byte_counter,
        )
        .await?;

        info!(server = %server_address, ingress_id = %ingress_id, "Connected to server.");

        Ok((tunnel, endpoint))
    }
}
