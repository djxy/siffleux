use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, Connection, Endpoint, VarInt};
use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

pub struct Tunnel {
    id: Uuid,
    server_address: SocketAddr,
    server_name: String,
    client_config: ClientConfig,
    connection: Mutex<Option<Connection>>,
}

impl Tunnel {
    pub fn new_with_self_signed_certificate(
        server_address: SocketAddr,
        server_name: String,
        certificate_der: CertificateDer,
    ) -> anyhow::Result<Tunnel> {
        let mut roots = RootCertStore::empty();

        roots.add(certificate_der)?;

        let crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        Ok(Tunnel::new(
            server_address,
            server_name,
            ClientConfig::new(Arc::new(QuicClientConfig::try_from(crypto)?)),
        ))
    }

    pub fn new(
        server_address: SocketAddr,
        server_name: String,
        client_config: ClientConfig,
    ) -> Self {
        Tunnel {
            id: Uuid::new_v4(),
            server_address,
            server_name,
            client_config,
            connection: Mutex::new(None),
        }
    }

    pub async fn connect(&self) -> anyhow::Result<()> {
        let mut connection_guard = self.connection.lock().await;

        if connection_guard.is_some() {
            return Err(anyhow::anyhow!("Connection already established."));
        }

        info!("Connecting {}", self.id);

        let connection = Endpoint::client(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?
            .connect_with(
                self.client_config.clone(),
                self.server_address,
                &self.server_name,
            )?
            .await?;

        let (mut send, mut recv) = connection.open_bi().await?;

        send.write_all(self.id.as_bytes()).await?;
        recv.read_to_end(0).await;
        send.finish()?;
        info!("Connected {}", self.id);

        *connection_guard = Some(connection);

        Ok(())
    }

    pub async fn close(&self) {
        if let Some(connection) = self.connection.lock().await.take() {
            connection.close(VarInt::from_u32(0), b"done");
        }
    }
}
