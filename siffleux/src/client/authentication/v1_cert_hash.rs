use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use quinn::{ClientConfig, Endpoint, TransportConfig, crypto::rustls::QuicClientConfig};
use tracing::info;

use crate::{
    AuthKey, Client, Egress, Error, Tunnel,
    client::{
        authentication::{
            authentication::Authentication, certificate_hash_verifier::CertificateHashVerifier,
        },
        protocols::v1::handle_client_protocol_v1_auth,
    },
    frames,
};

pub struct V1CertifcateHash {
    client: Client,
    auth_key: AuthKey,
    server_address: SocketAddr,
    server_name: String,
    certificate_hash: Vec<u8>,
}

impl V1CertifcateHash {
    pub fn new(
        client: Client,
        auth_key: AuthKey,
        server_address: SocketAddr,
        server_name: String,
        certificate_hash: Vec<u8>,
    ) -> Self {
        Self {
            client,
            auth_key,
            server_address,
            server_name,
            certificate_hash,
        }
    }
}

#[async_trait::async_trait]
impl Authentication for V1CertifcateHash {
    async fn connect(&self, egress: &dyn Egress) -> Result<Tunnel, Error> {
        let mut tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(CertificateHashVerifier::new(
                self.certificate_hash.clone(),
            )))
            .with_no_client_auth();

        tls_config.alpn_protocols = vec![frames::v1::VERSION.to_vec()];

        info!(server = %self.server_address, ingress_id = %egress.ingress_id(), "Connecting to server...");

        let mut transport_config = TransportConfig::default();

        transport_config.send_window(256 * 1024 * 1024);
        transport_config.receive_window((256 * 1024 * 1024u32).into());
        transport_config.stream_receive_window((2 * 1024 * 1024u32).into());

        transport_config.max_concurrent_bidi_streams(1000u32.into());

        let mut client_config =
            ClientConfig::new(Arc::new(QuicClientConfig::try_from(tls_config)?));

        client_config.transport_config(Arc::new(transport_config));

        let tunnel = handle_client_protocol_v1_auth(
            Endpoint::client(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0))?
                .connect_with(client_config, self.server_address, &self.server_name)?
                .await?,
            &self.auth_key,
            egress.ingress_id(),
            self.client.byte_counter(),
        )
        .await?;

        info!(server = %self.server_address, ingress_id = %egress.ingress_id(), "Connected to server.");

        Ok(tunnel)
    }
}
