use crate::server::tunnel_connection::TunnelConnection;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
pub struct TcpIngress {
    inner: Arc<TcpIngressInner>,
}

pub struct TcpIngressInner {
    tunnels_id: RwLock<HashMap<Uuid, Arc<TunnelConnection>>>,
}

impl Deref for TcpIngress {
    type Target = TcpIngressInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl TcpIngress {

}