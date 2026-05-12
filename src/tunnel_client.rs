use crate::tunnel_stream::TunnelStream;
use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, Connection, Endpoint, VarInt};
use rustls::pki_types::CertificateDer;
use rustls::RootCertStore;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct TunnelClient {
    server_address: SocketAddr,
    server_name: String,
    client_config: ClientConfig,
    connection: Mutex<Option<Connection>>,
}

impl TunnelClient {
    pub fn new_with_self_signed_certificate(
        server_address: SocketAddr,
        server_name: String,
        certificate_der: CertificateDer,
    ) -> anyhow::Result<TunnelClient> {
        let mut roots = RootCertStore::empty();

        roots.add(certificate_der)?;

        let crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        Ok(TunnelClient::new(
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
        TunnelClient {
            server_address,
            server_name,
            client_config,
            connection: Mutex::new(None),
        }
    }

    pub async fn connect(&mut self) -> anyhow::Result<()> {
        let mut connection_guard = self.connection.lock().await;

        if connection_guard.is_some() {
            return Err(anyhow::anyhow!("Connection already established."));
        }

        let connection = Endpoint::client(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?
            .connect_with(
                self.client_config.clone(),
                self.server_address,
                &self.server_name,
            )?
            .await?;

        *connection_guard = Some(connection);

        Ok(())
    }

    pub async fn create_tunnel(&mut self) -> anyhow::Result<TunnelStream> {
        match self.connection.lock().await.as_ref() {
            Some(connection) => {
                let (send, recv) = connection.open_bi().await?;

                Ok(TunnelStream::new(send, recv))
            }
            None => Err(anyhow::anyhow!("Not connected.")),
        }
    }

    pub async fn close(&mut self) -> anyhow::Result<()> {
        if let Some(connection) = self.connection.lock().await.as_ref() {
            connection.close(VarInt::from_u32(0), b"done");

            Ok(())
        } else {
            Err(anyhow::anyhow!("Not connected."))
        }
    }
}
