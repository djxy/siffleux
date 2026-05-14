use quinn::Connection;
use uuid::Uuid;

pub(in crate::server) struct TunnelConnection {
    id: Uuid,
    name: String,
    ingress_id: String,
    connection: Connection,
}

impl TunnelConnection {
    pub fn new(id: Uuid, name: String, ingress_id: String, connection: Connection) -> Self {
        Self {
            id,
            name,
            ingress_id,
            connection,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ingress_id(&self) -> &str {
        &self.ingress_id
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}
