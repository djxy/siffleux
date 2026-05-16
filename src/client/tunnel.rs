use crate::error::Error;
use crate::message::handshake::{HandshakeV1Request, HandshakeV1Response};
use crate::types::{AuthKey, IngressId, TunnelId, TunnelName};
use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, Connection, Endpoint, VarInt};
use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Deref;
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct Tunnel {
    inner: Arc<TunnelInner>,
}

pub struct TunnelInner {
    id: TunnelId,
    name: TunnelName,
    ingress_id: IngressId,
    connection: Connection,
}

impl Deref for Tunnel {
    type Target = TunnelInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Tunnel {
    pub async fn connect_with_certificates(
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
    ) -> Result<Tunnel, Error> {
        let (mut send, mut recv) = connection.open_bi().await?;

        info!("Sending handshake ingress_id={ingress_id}");

        HandshakeV1Request::write(&mut send, &auth_key, &ingress_id, &name).await?;

        let response = HandshakeV1Response::read(&mut recv).await?;

        recv.read_to_end(0).await?;

        info!(
            "Handshake complete ID={} ingress_id={ingress_id}.",
            response.tunnel_id
        );

        Ok(Tunnel::new(
            response.tunnel_id,
            name,
            ingress_id,
            connection,
        ))
    }

    fn new(
        id: TunnelId,
        name: TunnelName,
        ingress_id: IngressId,
        connection: Connection,
    ) -> Self {
        Tunnel {
            inner: Arc::new(TunnelInner {
                id,
                name,
                ingress_id,
                connection,
            }),
        }
    }

    pub fn id(&self) -> TunnelId {
        self.id
    }

    pub fn name(&self) -> &TunnelName {
        &self.name
    }

    pub fn ingress_id(&self) -> &IngressId {
        &self.ingress_id
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn close(&self) {
        self.connection.close(VarInt::from_u32(0), b"done");
    }
}
