use crate::common::error::Error;
use crate::common::message::code::Code;
use crate::common::tunnel_stream::TunnelStream;
use crate::common::types::{IngressId, TunnelId, TunnelName};
use quinn::{Connection, ConnectionError};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

pub struct Tunnel {
    state: TunnelState,
    connection: Connection,
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
    is_closed: RwLock<Option<ConnectionError>>,
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
        let connection = self.connection.clone();

        tokio::spawn(async move {
            let reason = connection.clone().closed().await;
        });
    }

    pub fn state(&self) -> &TunnelState {
        &self.state
    }

    pub fn close(&self, code: &Code) {
        self.connection.close(code.code, code.reason);
    }

    pub async fn create_stream(&self) -> Result<TunnelStream, Error> {
        let (send, recv) = self.connection.open_bi().await?;

        Ok(TunnelStream::new(send, recv))
    }
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
