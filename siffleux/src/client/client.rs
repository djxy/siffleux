use std::{
    net::{SocketAddr, UdpSocket},
    sync::Arc,
};

use quinn::{
    ClientConfig, Endpoint, TransportConfig,
    crypto::rustls::QuicClientConfig,
    udp::{UdpSockRef, UdpSocketState},
};
use socket2::{Domain, Protocol, Socket, Type};
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

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

        socket.set_send_buffer_size(8 * 1024 * 1024)?;
        socket.set_recv_buffer_size(8 * 1024 * 1024)?;
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;

        let udp_state = UdpSocketState::new(UdpSockRef::from(&socket))?;

        info!("Max GSO segments: {}", udp_state.max_gso_segments());
        info!("GRO segments:     {}", udp_state.gro_segments());
        info!("May fragment:     {}", udp_state.may_fragment());

        let std_socket: UdpSocket = socket.into();

        let endpoint = quinn::Endpoint::new(
            quinn::EndpointConfig::default(),
            None,
            std_socket,
            Arc::new(quinn::TokioRuntime),
        )?;

        let tunnel = handle_client_protocol_v1_auth(
            endpoint
                .connect_with(client_config, server_address, &server_name)?
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
