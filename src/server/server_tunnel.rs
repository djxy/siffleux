use crate::types::{IngressId, TunnelId, TunnelName};
use quinn::Connection;
use std::ops::Deref;
use std::sync::Arc;

/// Tunnel representation on the server.
#[derive(Clone)]
pub struct ServerTunnel {
    inner: Arc<ServerTunnelInner>,
}

pub struct ServerTunnelInner {
    id: TunnelId,
    name: TunnelName,
    ingress_id: IngressId,
    connection: Connection,
}

impl ServerTunnel {
    pub fn new(
        id: TunnelId,
        name: TunnelName,
        ingress_id: IngressId,
        connection: Connection,
    ) -> Self {
        Self {
            inner: Arc::from(ServerTunnelInner {
                id,
                name,
                ingress_id,
                connection,
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

    pub fn connection(&self) -> &Connection {
        &self.inner.connection
    }
}
