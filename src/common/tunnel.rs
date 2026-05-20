use crate::common::error::Error;
use crate::common::message::code::Code;
use crate::common::types::{IngressId, TunnelId, TunnelName};
use quinn::Connection;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

pub struct Tunnel {
    state: TunnelState,
    connection: Connection,
}

impl Tunnel {
    pub fn new(
        id: TunnelId,
        name: TunnelName,
        ingress_id: IngressId,
        connection: Connection,
    ) -> Self {
        let state = TunnelState::new(id, name, ingress_id);

        Self { state, connection }
    }

    pub fn start_hooks(&self) {
        self.start_close_hook();
    }

    pub fn state(&self) -> &TunnelState {
        &self.state
    }

    pub fn close(&self, code: &Code) {
        self.connection.close(code.code, code.reason);
    }

    pub async fn create_stream(&self) -> Result<(ReadStream, SendStream), Error> {
        let (send, recv) = self.connection.open_bi().await?;

        Ok((
            ReadStream::new(recv, self.state.clone()),
            SendStream::new(send, self.state.clone()),
        ))
    }

    pub async fn accept_stream(&self) -> Result<(ReadStream, SendStream), Error> {
        let (send, recv) = self.connection.accept_bi().await?;

        Ok((
            ReadStream::new(recv, self.state.clone()),
            SendStream::new(send, self.state.clone()),
        ))
    }

    fn start_close_hook(&self) {
        let state = self.state.clone();
        let connection = self.connection.clone();

        tokio::spawn(async move {
            let reason = connection.clone().closed().await;
            let _ = state.inner.is_closed.write().unwrap().insert(reason.into());
        });
    }
}

pub struct ReadStream {
    stream: quinn::RecvStream,
    tunnel_state: TunnelState,
}

impl ReadStream {
    pub fn new(stream: quinn::RecvStream, tunnel_state: TunnelState) -> ReadStream {
        ReadStream {
            stream,
            tunnel_state,
        }
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, Error> {
        let size_opt = self.stream.read(buf).await?;

        if let Some(size) = size_opt {
            self.tunnel_state
                .inner
                .bytes_received
                .fetch_add(size as u64, Ordering::Relaxed);
        }

        Ok(size_opt)
    }
}

pub struct SendStream {
    stream: quinn::SendStream,
    tunnel_state: TunnelState,
}

impl SendStream {
    pub fn new(stream: quinn::SendStream, tunnel_state: TunnelState) -> SendStream {
        SendStream {
            stream,
            tunnel_state,
        }
    }

    pub async fn write(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let size = self.stream.write(buf).await?;

        self.tunnel_state
            .inner
            .bytes_sent
            .fetch_add(size as u64, Ordering::Relaxed);

        Ok(size)
    }
}

#[derive(Clone)]
pub struct TunnelState {
    inner: Arc<TunnelStateInner>,
}

struct TunnelStateInner {
    id: TunnelId,
    name: TunnelName,
    ingress_id: IngressId,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    is_closed: RwLock<Option<Error>>,
}

impl TunnelState {
    fn new(id: TunnelId, name: TunnelName, ingress_id: IngressId) -> Self {
        Self {
            inner: Arc::new(TunnelStateInner {
                id,
                name,
                ingress_id,
                bytes_sent: AtomicU64::new(0),
                bytes_received: AtomicU64::new(0),
                is_closed: RwLock::new(None),
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
        self.inner.is_closed.read().unwrap().is_some()
    }
}
