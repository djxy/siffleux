use crate::messages::{HandshakeV1Request, HandshakeV1Response};
use crate::{AuthKey, Error, IngressId, Tunnel, TunnelId, TunnelName};
use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, Connection, Endpoint};
use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

struct ClientInner {
    tunnel: Tunnel,
}

impl Client {
    pub async fn connect_with_certificates(
        auth_key: AuthKey,
        ingress_id: IngressId,
        name: TunnelName,
        server_address: SocketAddr,
        server_name: String,
        certificates: Vec<CertificateDer<'static>>,
    ) -> Result<Client, Error> {
        let mut roots = RootCertStore::empty();

        for cert in certificates {
            roots.add(cert)?;
        }

        let crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        info!("Connecting ingress_id={ingress_id}");

        Self::complete_handshake(
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
        )
        .await
    }

    async fn complete_handshake(
        connection: Connection,
        auth_key: AuthKey,
        ingress_id: IngressId,
        name: TunnelName,
    ) -> Result<Client, Error> {
        let (mut send, mut recv) = connection.open_bi().await?;

        info!("Sending handshake ingress_id={ingress_id}");

        HandshakeV1Request::write(&mut send, &auth_key, &ingress_id, &name).await?;

        let response = HandshakeV1Response::read(&mut recv).await?;

        recv.read_to_end(0).await?;

        info!(
            "Handshake complete ID={} ingress_id={ingress_id}.",
            response.tunnel_id
        );

        Ok(Client::new(
            response.tunnel_id,
            name,
            ingress_id,
            connection,
        ))
    }

    fn new(id: TunnelId, name: TunnelName, ingress_id: IngressId, connection: Connection) -> Self {
        Client {
            inner: Arc::new(ClientInner {
                tunnel: Tunnel::new(id, name, ingress_id, connection),
            }),
        }
    }

    pub fn tunnel(&self) -> &Tunnel {
        &self.inner.tunnel
    }
}
