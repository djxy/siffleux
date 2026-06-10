use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, StreamId};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::CertificateDer;
use rustls::{DigitallySignedStruct, RootCertStore, SignatureScheme};
use sha2::{Digest, Sha256};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::io::{InspectReader, InspectWriter};
use tracing::info;

use crate::frames::v1::CodecV1;
use crate::{AuthKey, Error, IngressId, TunnelId, TunnelName};

#[derive(Clone)]
pub struct Tunnel {
    inner: Arc<TunnelInner>,
}

pub struct TunnelInner {
    connection: Connection,
    id: TunnelId,
    name: TunnelName,
    ingress_id: IngressId,
    bytes_sent: Arc<AtomicUsize>,
    bytes_received: Arc<AtomicUsize>,
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
                bytes_sent: Arc::new(AtomicUsize::new(0)),
                bytes_received: Arc::new(AtomicUsize::new(0)),
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

    pub fn bytes_sent(&self) -> usize {
        self.inner.bytes_sent.load(Ordering::Relaxed)
    }

    pub fn bytes_received(&self) -> usize {
        self.inner.bytes_received.load(Ordering::Relaxed)
    }

    pub fn connection(&self) -> &Connection {
        &self.inner.connection
    }

    pub async fn create_stream(
        &self,
    ) -> Result<
        (
            FramedRead<InspectReader<RecvStream, impl FnMut(&[u8])>, CodecV1>,
            FramedWrite<InspectWriter<SendStream, impl FnMut(&[u8])>, CodecV1>,
            Stream,
        ),
        Error,
    > {
        let (send_stream, recv_stream) = self.inner.connection.open_bi().await?;

        self.quinn_stream_to_framed(send_stream, recv_stream)
    }

    pub async fn accept_stream(
        &self,
    ) -> Result<
        (
            FramedRead<InspectReader<RecvStream, impl FnMut(&[u8])>, CodecV1>,
            FramedWrite<InspectWriter<SendStream, impl FnMut(&[u8])>, CodecV1>,
            Stream,
        ),
        Error,
    > {
        let (send_stream, recv_stream) = self.inner.connection.accept_bi().await?;

        self.quinn_stream_to_framed(send_stream, recv_stream)
    }

    fn quinn_stream_to_framed(
        &self,
        send_stream: SendStream,
        recv_stream: RecvStream,
    ) -> Result<
        (
            FramedRead<InspectReader<RecvStream, impl FnMut(&[u8])>, CodecV1>,
            FramedWrite<InspectWriter<SendStream, impl FnMut(&[u8])>, CodecV1>,
            Stream,
        ),
        Error,
    > {
        let stream_bytes_received = Arc::new(AtomicUsize::new(0));
        let stream_bytes_sent = Arc::new(AtomicUsize::new(0));
        let stream = Stream::new(
            send_stream.id(),
            stream_bytes_sent.clone(),
            stream_bytes_received.clone(),
        );

        let tunnel_bytes_received = self.inner.bytes_received.clone();
        let tunnel_bytes_sent = self.inner.bytes_sent.clone();

        let read = FramedRead::new(
            InspectReader::new(recv_stream, move |bytes| {
                let bytes_len = bytes.len();

                stream_bytes_received.fetch_add(bytes_len, Ordering::Relaxed);
                tunnel_bytes_received.fetch_add(bytes_len, Ordering::Relaxed);
            }),
            CodecV1,
        );
        let send = FramedWrite::new(
            InspectWriter::new(send_stream, move |bytes| {
                let bytes_len = bytes.len();

                stream_bytes_sent.fetch_add(bytes_len, Ordering::Relaxed);
                tunnel_bytes_sent.fetch_add(bytes_len, Ordering::Relaxed);
            }),
            CodecV1,
        );

        Ok((read, send, stream))
    }
}

#[derive(Clone)]
pub struct Stream {
    inner: Arc<StreamInner>,
}

struct StreamInner {
    id: StreamId,
    bytes_sent: Arc<AtomicUsize>,
    bytes_received: Arc<AtomicUsize>,
}

impl Stream {
    fn new(id: StreamId, bytes_sent: Arc<AtomicUsize>, bytes_received: Arc<AtomicUsize>) -> Self {
        Stream {
            inner: Arc::new(StreamInner {
                id,
                bytes_sent,
                bytes_received,
            }),
        }
    }

    pub fn id(&self) -> StreamId {
        self.inner.id
    }

    pub fn bytes_sent(&self) -> usize {
        self.inner.bytes_sent.load(Ordering::Relaxed)
    }

    pub fn bytes_received(&self) -> usize {
        self.inner.bytes_received.load(Ordering::Relaxed)
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
