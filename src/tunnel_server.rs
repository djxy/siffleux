use quinn::{Endpoint, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::net::SocketAddr;
use std::sync::Arc;

pub struct TunnelServer {
    server_config: ServerConfig,
}

impl TunnelServer {
    pub fn new_with_self_signed_certificate(
        certificate_der: CertificateDer<'static>,
        private_key: PrivatePkcs8KeyDer<'static>,
    ) -> anyhow::Result<TunnelServer> {
        let crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate_der], PrivateKeyDer::from(private_key))?;

        let server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(crypto)?,
        ));

        Ok(TunnelServer::new(server_config))
    }

    pub fn new(server_config: ServerConfig) -> TunnelServer {
        TunnelServer { server_config }
    }

    pub fn listen(&self, listen_address: SocketAddr) -> anyhow::Result<Endpoint> {
        Ok(Endpoint::server(
            self.server_config.clone(),
            listen_address,
        )?)
    }
}
