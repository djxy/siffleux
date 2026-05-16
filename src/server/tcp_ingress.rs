use crate::server::server_tunnel::ServerTunnel;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct TcpIngress {
    inner: Arc<TcpIngressInner>,
}

pub struct TcpIngressInner {
    tunnels_id: RwLock<HashMap<u64, Arc<ServerTunnel>>>,
}

impl Deref for TcpIngress {
    type Target = TcpIngressInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl TcpIngress {

}