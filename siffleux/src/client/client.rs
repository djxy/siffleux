use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use quinn::{ClientConfig, Endpoint, crypto::rustls::QuicClientConfig};
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

        let endpoint = Endpoint::client(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))?;
        let tunnel = handle_client_protocol_v1_auth(
            endpoint
                .connect_with(
                    ClientConfig::new(Arc::new(QuicClientConfig::try_from(tls_config)?)),
                    server_address,
                    &server_name,
                )?
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
