use quinn::Connection;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_util::sync::CancellationToken;

use crate::codes::CLOSED;
use crate::{Code, Error, IngressId, StreamId, TunnelId, TunnelName};

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
    stream_id_counter: AtomicU64,
}

impl Tunnel {
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
                stream_id_counter: AtomicU64::new(0),
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
        let stream = Stream::new(
            self.clone(),
            StreamId::new(self.inner.stream_id_counter.fetch_add(1, Ordering::SeqCst)),
        );

        Ok((
            ReadChannel::new(recv, stream.clone()),
            WriteChannel::new(send, stream),
        ))
    }

    pub async fn accept_stream(&self) -> Result<(ReadChannel, WriteChannel), Error> {
        let (send, recv) = self.inner.connection.accept_bi().await?;
        let stream = Stream::new(
            self.clone(),
            StreamId::new(self.inner.stream_id_counter.fetch_add(1, Ordering::SeqCst)),
        );

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
