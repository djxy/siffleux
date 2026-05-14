use crate::messages::handshake::{HandshakeV1Request, HandshakeV1Response};
use crate::server::KipawaServer;
use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, Connection, Endpoint, VarInt};
use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Deref;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
pub struct KipawaTunnel {
    inner: Arc<KipawaTunnelInner>,
}

pub struct KipawaTunnelInner {
    id: Uuid,
    name: String,
    ingress_id: String,
    connection: Connection,
}

impl Deref for KipawaTunnel {
    type Target = KipawaTunnelInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl KipawaTunnel {
    pub async fn connect_with_certificates(
        auth_key: &str,
        ingress_id: &str,
        name: &str,
        server_address: SocketAddr,
        server_name: String,
        certificates: Vec<CertificateDer<'static>>,
    ) -> anyhow::Result<KipawaTunnel> {
        let mut roots = RootCertStore::empty();

        for cert in certificates {
            roots.add(cert)?;
        }

        let crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        info!("Connecting auth_key={auth_key} ingress_id={ingress_id}");

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
        auth_key: &str,
        ingress_id: &str,
        name: &str,
    ) -> anyhow::Result<KipawaTunnel> {
        let (mut send, mut recv) = connection.open_bi().await?;

        info!("Sending handshake auth_key={auth_key} ingress_id={ingress_id}");

        HandshakeV1Request::write(&mut send, auth_key, ingress_id, name).await?;

        let response = HandshakeV1Response::read(&mut recv).await?;

        recv.read_to_end(0).await?;

        info!(
            "Handshake complete ID={} auth_key={auth_key} ingress_id={ingress_id}.",
            response.id
        );

        Ok(KipawaTunnel::new(response.id, name, ingress_id, connection))
    }

    fn new(id: Uuid, name: &str, ingress_id: &str, connection: Connection) -> Self {
        KipawaTunnel {
            inner: Arc::new(KipawaTunnelInner {
                id,
                name: name.to_string(),
                ingress_id: ingress_id.to_string(),
                connection,
            }),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ingress_id(&self) -> &str {
        &self.ingress_id
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn close(&self) {
        self.connection.close(VarInt::from_u32(0), b"done");
    }
}
