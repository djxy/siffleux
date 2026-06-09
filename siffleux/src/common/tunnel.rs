use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, Connection, Endpoint};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::CertificateDer;
use rustls::{DigitallySignedStruct, RootCertStore, SignatureScheme};
use sha2::{Digest, Sha256};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::codes::CLOSED;
use crate::{AuthKey, Code, Error, IngressId, StreamId, TunnelId, TunnelName};

#[derive(Clone)]
pub struct Tunnel {
    inner: Arc<TunnelInner>,
}

pub struct TunnelInner {
    connection: Connection,
    id: TunnelId,
    name: TunnelName,
    ingress_id: IngressId,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
}

impl Tunnel {
    pub async fn connect_to_server_with_certificate_hash(
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

        info!("Connecting to server ingress_id={ingress_id} with certificate hash verification.");

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

    pub async fn connect_to_server_with_certificates(
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

        info!("Connecting to server ingress_id={ingress_id} with certificate(s).");

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

        info!("Sending handshake to ingress_id={ingress_id}");

        authentication::v1::Request::write(&mut send, &auth_key, &ingress_id, &name).await?;

        let response = authentication::v1::Response::read(&mut recv).await?;

        recv.read_to_end(0).await?;

        info!(
            "Handshake complete. Received tunnel_id={} on ingress_id={ingress_id}",
            response.tunnel_id
        );

        Ok(Tunnel::new(
            response.tunnel_id,
            name,
            ingress_id,
            connection,
        ))
    }

    pub fn new(
        id: TunnelId,
        name: TunnelName,
        ingress_id: IngressId,
        connection: Connection,
    ) -> Self {
        Self {
            inner: Arc::new(TunnelInner {
                connection,
                id,
                name,
                ingress_id,
                bytes_sent: AtomicU64::new(0),
                bytes_received: AtomicU64::new(0),
            }),
        }
    }

    pub fn id(&self) -> TunnelId {
        self.inner.id
    }

    pub fn name(&self) -> &TunnelName {
        &self.inner.name
    }

    pub fn ingress_id(&self) -> &IngressId {
        &self.inner.ingress_id
    }

    pub fn bytes_sent(&self) -> u64 {
        self.inner.bytes_sent.load(Ordering::Relaxed)
    }

    pub fn bytes_received(&self) -> u64 {
        self.inner.bytes_received.load(Ordering::Relaxed)
    }

    pub fn is_closed(&self) -> bool {
        self.inner.connection.close_reason().is_some()
    }

    /// Wait for the tunnel to close. Even if it returns an Error, it is the reason when the connection closed.
    pub async fn closed(&self) -> Error {
        self.inner.connection.closed().await.into()
    }

    pub fn close(&self, code: &Code) {
        self.inner.connection.close(code.code, code.reason);
    }

    pub async fn create_stream(&self) -> Result<(ReadChannel, WriteChannel), Error> {
        let (send, recv) = self.inner.connection.open_bi().await?;
        let stream = Stream::new(self.clone(), StreamId::new(send.id().index()));

        Ok((
            ReadChannel::new(recv, stream.clone()),
            WriteChannel::new(send, stream),
        ))
    }

    pub async fn accept_stream(&self) -> Result<(ReadChannel, WriteChannel), Error> {
        let (send, recv) = self.inner.connection.accept_bi().await?;
        let stream = Stream::new(self.clone(), StreamId::new(send.id().index()));

        Ok((
            ReadChannel::new(recv, stream.clone()),
            WriteChannel::new(send, stream),
        ))
    }
}

#[derive(Clone)]
pub struct Stream {
    inner: Arc<StreamInner>,
}

struct StreamInner {
    id: StreamId,
    tunnel: Tunnel,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    closed_token: CancellationToken,
}

impl Stream {
    fn new(tunnel: Tunnel, id: StreamId) -> Self {
        Stream {
            inner: Arc::new(StreamInner {
                id,
                tunnel,
                bytes_sent: AtomicU64::new(0),
                bytes_received: AtomicU64::new(0),
                closed_token: CancellationToken::new(),
            }),
        }
    }

    pub fn id(&self) -> StreamId {
        self.inner.id
    }

    pub fn bytes_sent(&self) -> u64 {
        self.inner.bytes_sent.load(Ordering::Relaxed)
    }

    pub fn bytes_received(&self) -> u64 {
        self.inner.bytes_received.load(Ordering::Relaxed)
    }

    fn close(&self) {
        self.inner.closed_token.cancel();
    }

    pub fn is_closed(&self) -> bool {
        self.inner.closed_token.is_cancelled()
    }

    pub async fn closed(&self) {
        self.inner.closed_token.cancelled().await;
    }

    fn increment_bytes_sent(&self, bytes: u64) {
        self.inner
            .tunnel
            .inner
            .bytes_sent
            .fetch_add(bytes, Ordering::Relaxed);

        self.inner.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    fn increment_bytes_received(&self, bytes: u64) {
        self.inner
            .tunnel
            .inner
            .bytes_received
            .fetch_add(bytes, Ordering::Relaxed);

        self.inner
            .bytes_received
            .fetch_add(bytes, Ordering::Relaxed);
    }
}

pub struct ReadChannel {
    quinn_stream: quinn::RecvStream,
    stream: Stream,
}

impl ReadChannel {
    pub fn new(quinn_stream: quinn::RecvStream, stream: Stream) -> ReadChannel {
        ReadChannel {
            quinn_stream,
            stream,
        }
    }

    pub fn stream(&self) -> &Stream {
        &self.stream
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, Error> {
        let size_opt = self.quinn_stream.read(buf).await?;

        if let Some(size) = size_opt {
            self.stream.increment_bytes_received(size as u64);
        }

        Ok(size_opt)
    }

    pub fn close(&mut self) -> Result<(), Error> {
        self.stream.close();
        Ok(self.quinn_stream.stop(CLOSED.code)?)
    }
}

pub struct WriteChannel {
    quinn_stream: quinn::SendStream,
    stream: Stream,
}

impl WriteChannel {
    pub fn new(quinn_stream: quinn::SendStream, stream: Stream) -> WriteChannel {
        WriteChannel {
            quinn_stream,
            stream,
        }
    }

    pub fn stream(&self) -> &Stream {
        &self.stream
    }

    /// Write a buffer into this stream, returning how many bytes were written
    ///
    /// # Cancel safety
    ///
    /// This method is cancellation safe. If this does not resolve, no bytes were written.
    pub async fn write(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let size = self.quinn_stream.write(buf).await?;

        self.stream.increment_bytes_sent(size as u64);

        Ok(size)
    }

    pub fn close(&mut self) -> Result<(), Error> {
        self.stream.close();
        Ok(self.quinn_stream.finish()?)
    }
}

#[derive(Debug)]
struct CertificateHashVerifier {
    certificate_hash: Vec<u8>,
    supported_signature_schemes: Vec<SignatureScheme>,
}

impl CertificateHashVerifier {
    fn new(certificate_hash: Vec<u8>) -> Self {
        Self {
            certificate_hash,
            supported_signature_schemes: CryptoProvider::get_default()
                .unwrap()
                .signature_verification_algorithms
                .supported_schemes(),
        }
    }
}

impl ServerCertVerifier for CertificateHashVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let got = Sha256::digest(end_entity.as_ref()).to_vec();
        if got == self.certificate_hash {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("certificate hash mismatch".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &CryptoProvider::get_default()
                .unwrap()
                .signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &CryptoProvider::get_default()
                .unwrap()
                .signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.supported_signature_schemes.clone()
    }
}
