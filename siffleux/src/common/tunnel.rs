use quinn::{Connection, RecvStream, SendStream, VarInt};
use std::sync::Arc;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::io::{InspectReader, InspectWriter};
use uuid::Uuid;

use crate::code::CONNECTION_EOF;
use crate::common::byte_counter::ByteCounter;
use crate::common::{IngressId, TunnelName};
use crate::frames::v1::CodecV1;
use crate::{Error, ServerId};

#[derive(Clone, Debug)]
pub struct Tunnel {
    inner: Arc<TunnelInner>,
}

#[derive(Debug)]
struct TunnelInner {
    id: Uuid,
    server_id: ServerId,
    connection: Connection,
    name: TunnelName,
    ingress_id: IngressId,
    byte_counter: ByteCounter,
}

impl Tunnel {
    pub fn new(
        id: Uuid,
        server_id: ServerId,
        name: TunnelName,
        ingress_id: IngressId,
        connection: Connection,
        parent_byte_counter: Option<ByteCounter>,
    ) -> Self {
        Self {
            inner: Arc::new(TunnelInner {
                id,
                server_id,
                connection,
                name,
                ingress_id,
                byte_counter: ByteCounter::new(parent_byte_counter),
            }),
        }
    }

    pub fn id(&self) -> Uuid {
        self.inner.id
    }

    pub fn server_id(&self) -> &ServerId {
        &self.inner.server_id
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

    pub async fn close_with_reason(&self, code: VarInt, reason: &[u8]) {
        self.inner.connection.close(code, reason);
        self.inner.connection.closed().await;
    }

    pub async fn close(&self) {
        self.close_with_reason(CONNECTION_EOF, b"done").await;
    }

    pub async fn create_stream(
        &self,
    ) -> Result<(TunnelReadStream, TunnelWriteStream, TunnelStream), Error> {
        let (send_stream, recv_stream) = self.inner.connection.open_bi().await?;

        self.quinn_stream_to_framed(send_stream, recv_stream)
    }

    pub async fn accept_stream(
        &self,
    ) -> Result<(TunnelReadStream, TunnelWriteStream, TunnelStream), Error> {
        let (send_stream, recv_stream) = self.inner.connection.accept_bi().await?;

        self.quinn_stream_to_framed(send_stream, recv_stream)
    }

    fn quinn_stream_to_framed(
        &self,
        send_stream: SendStream,
        recv_stream: RecvStream,
    ) -> Result<(TunnelReadStream, TunnelWriteStream, TunnelStream), Error> {
        let stream = TunnelStream::new(send_stream.id().index(), self.inner.byte_counter.clone());

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
pub struct TunnelStream {
    inner: Arc<TunnelStreamInner>,
}

struct TunnelStreamInner {
    id: u64,
    byte_counter: ByteCounter,
}

impl TunnelStream {
    fn new(id: u64, tunnel_byte_counter: ByteCounter) -> Self {
        TunnelStream {
            inner: Arc::new(TunnelStreamInner {
                id,
                byte_counter: ByteCounter::new(Some(tunnel_byte_counter)),
            }),
        }
    }

    pub fn id(&self) -> u64 {
        self.inner.id
    }

    pub fn byte_counter(&self) -> &ByteCounter {
        &self.inner.byte_counter
    }
}
