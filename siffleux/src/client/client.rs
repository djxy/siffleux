use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use quinn::{ClientConfig, Connection, Endpoint, crypto::rustls::QuicClientConfig};
use rustls::{RootCertStore, pki_types::CertificateDer};
use tracing::debug;

use crate::{
    Error, Tunnel,
    client::{
        certificate_verifier::CertificateHashVerifier, protocols::v1::handle_protocol_v1_auth,
    },
    common::{AuthKey, ByteCounter, IngressId, TunnelName},
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

    pub async fn create_tcp_tunnel(
        auth_key: AuthKey,
        ingress_id: IngressId,
        name: TunnelName,
        server_address: SocketAddr,
        server_name: String,
        certificate_hash: Vec<u8>,
    ) -> Result<Tunnel, Error> {
        let verifier = Arc::new(CertificateHashVerifier::new(certificate_hash));

        let tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();

        debug!("Connecting to server ingress_id={ingress_id} with certificate hash verification.");

        Self::complete_handshake(
            Endpoint::client(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))?
                .connect_with(
                    ClientConfig::new(Arc::new(QuicClientConfig::try_from(tls_config)?)),
                    server_address,
                    &server_name,
                )?
                .await?,
            auth_key,
            ingress_id,
            name,
        )
        .await
    }

    pub async fn connect_to_server(
        &self,
        auth_key: AuthKey,
        ingress_id: IngressId,
        name: TunnelName,
        server_address: SocketAddr,
        server_name: String,
        certificates: Vec<CertificateDer<'static>>,
    ) -> Result<Tunnel, Error> {
        let mut roots = RootCertStore::empty();

        for cert in certificates {
            roots.add(cert)?;
        }

        let crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        debug!("Connecting to server ingress_id={ingress_id} with certificate(s).");

        handle_protocol_v1_auth(
            Endpoint::client(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?
                .connect_with(
                    ClientConfig::new(Arc::new(QuicClientConfig::try_from(crypto)?)),
                    server_address,
                    &server_name,
                )?
                .await?,
            auth_key,
            ingress_id,
            name,
            &self.inner.byte_counter,
        )
        .await
    }

    async fn connect_to_server() {}
}
