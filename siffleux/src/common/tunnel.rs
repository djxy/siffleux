use quinn::{Connection, RecvStream, SendStream, StreamId};
use std::sync::Arc;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::io::{InspectReader, InspectWriter};

use crate::Error;
use crate::code::CONNECTION_EOF;
use crate::common::byte_counter::ByteCounter;
use crate::common::{IngressId, TunnelId, TunnelName};
use crate::frames::v1::CodecV1;

#[derive(Clone, Debug)]
pub struct Tunnel {
    inner: Arc<TunnelInner>,
}

#[derive(Debug)]
struct TunnelInner {
    connection: Connection,
    id: TunnelId,
    name: TunnelName,
    ingress_id: IngressId,
    byte_counter: ByteCounter,
}

impl Tunnel {
    pub fn new(
        id: TunnelId,
        name: TunnelName,
        ingress_id: IngressId,
        connection: Connection,
        parent_byte_counter: Option<ByteCounter>,
    ) -> Self {
        Self {
            inner: Arc::new(TunnelInner {
                connection,
                id,
                name,
                ingress_id,
                byte_counter: ByteCounter::new(parent_byte_counter),
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

    pub fn byte_counter(&self) -> &ByteCounter {
        &self.inner.byte_counter
    }

    pub fn is_closed(&self) -> bool {
        self.inner.connection.close_reason().is_some()
    }

    pub async fn closed(&self) {
        self.inner.connection.closed().await;
    }

    pub fn close(&self) {
        self.inner.connection.close(CONNECTION_EOF, b"done");
    }

    pub async fn create_stream(
        &self,
    ) -> Result<(TunnelReadStream, TunnelWriteStream, Stream), Error> {
        let (send_stream, recv_stream) = self.inner.connection.open_bi().await?;

        self.quinn_stream_to_framed(send_stream, recv_stream)
    }

    pub async fn accept_stream(
        &self,
    ) -> Result<(TunnelReadStream, TunnelWriteStream, Stream), Error> {
        let (send_stream, recv_stream) = self.inner.connection.accept_bi().await?;

        self.quinn_stream_to_framed(send_stream, recv_stream)
    }

    fn quinn_stream_to_framed(
        &self,
        send_stream: SendStream,
        recv_stream: RecvStream,
    ) -> Result<(TunnelReadStream, TunnelWriteStream, Stream), Error> {
        let stream = Stream::new(send_stream.id(), self.inner.byte_counter.clone());

        let stream_read_byte_counter = stream.byte_counter().clone();
        let stream_write_byte_counter = stream.byte_counter().clone();

        Ok((
            InspectReader::new(
                recv_stream,
                Box::new(move |bytes| {
                    stream_read_byte_counter.add_bytes_read(bytes.len());
                }),
            ),
            InspectWriter::new(
                send_stream,
                Box::new(move |bytes| {
                    stream_write_byte_counter.add_bytes_write(bytes.len());
                }),
            ),
            stream,
        ))
    }
}

pub type TunnelReadStream = InspectReader<RecvStream, Box<dyn FnMut(&[u8]) + Send>>;
pub type TunnelWriteStream = InspectWriter<SendStream, Box<dyn FnMut(&[u8]) + Send>>;

pub type TunnelReadFramed = FramedRead<TunnelReadStream, CodecV1>;
pub type TunnelWriteFramed = FramedWrite<TunnelWriteStream, CodecV1>;

#[derive(Clone)]
pub struct Stream {
    inner: Arc<StreamInner>,
}

struct StreamInner {
    id: StreamId,
    byte_counter: ByteCounter,
}

impl Stream {
    fn new(id: StreamId, tunnel_byte_counter: ByteCounter) -> Self {
        Stream {
            inner: Arc::new(StreamInner {
                id,
                byte_counter: ByteCounter::new(Some(tunnel_byte_counter)),
            }),
        }
    }

    pub fn id(&self) -> StreamId {
        self.inner.id
    }

    pub fn byte_counter(&self) -> &ByteCounter {
        &self.inner.byte_counter
    }
}
