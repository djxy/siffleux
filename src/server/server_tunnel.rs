use crate::types::{IngressId, TunnelId, TunnelName};
use quinn::Connection;

/// Tunnel representation on the server.
pub struct ServerTunnel {
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
            id,
            name,
            ingress_id,
            connection,
        }
    }

    pub fn id(&self) -> TunnelId {
        self.id
    }

    pub fn name(&self) -> &TunnelName {
        &self.name
    }

    pub fn ingress_id(&self) -> &IngressId {
        &self.ingress_id
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}
