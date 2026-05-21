use crate::common::error::Error;
use crate::common::message::code::Code;
use crate::common::types::{IngressId, TunnelId, TunnelName};
use quinn::Connection;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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

    pub async fn create_stream(&self) -> Result<(ReadStream, WriteStream), Error> {
        let (send, recv) = self.inner.connection.open_bi().await?;

        Ok((
            ReadStream::new(recv, self.clone()),
            WriteStream::new(send, self.clone()),
        ))
    }

    pub async fn accept_stream(&self) -> Result<(ReadStream, WriteStream), Error> {
        let (send, recv) = self.inner.connection.accept_bi().await?;

        Ok((
            ReadStream::new(recv, self.clone()),
            WriteStream::new(send, self.clone()),
        ))
    }
}

pub struct ReadStream {
    stream: quinn::RecvStream,
    tunnel: Tunnel,
}

impl ReadStream {
    pub fn new(stream: quinn::RecvStream, tunnel: Tunnel) -> ReadStream {
        ReadStream { stream, tunnel }
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, Error> {
        let size_opt = self.stream.read(buf).await?;

        if let Some(size) = size_opt {
            self.tunnel
                .inner
                .bytes_received
                .fetch_add(size as u64, Ordering::Relaxed);
        }

        Ok(size_opt)
    }
}

pub struct WriteStream {
    stream: quinn::SendStream,
    tunnel: Tunnel,
}

impl WriteStream {
    pub fn new(stream: quinn::SendStream, tunnel: Tunnel) -> WriteStream {
        WriteStream { stream, tunnel }
    }

    /// Write a buffer into this stream, returning how many bytes were written
    ///
    /// # Cancel safety
    ///
    /// This method is cancellation safe. If this does not resolve, no bytes were written.
    pub async fn write(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let size = self.stream.write(buf).await?;

        self.tunnel
            .inner
            .bytes_sent
            .fetch_add(size as u64, Ordering::Relaxed);

        Ok(size)
    }
}
