use crate::server::ingress::ingress::Ingress;
use crate::server::server_tunnel::ServerTunnel;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct MockIngress {
    inner: Arc<MockIngressInner>,
}

pub struct MockIngressInner {
    tunnels_connected: RwLock<Vec<ServerTunnel>>,
}

impl Deref for MockIngress {
    type Target = MockIngressInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Ingress for MockIngress {
    fn on_tunnel_connected(&self, tunnel: ServerTunnel) {
        let self_clone = self.clone();

        tokio::spawn(async move {
            self_clone
                .inner
                .tunnels_connected
                .write()
                .await
                .push(tunnel);
        });
    }
}

impl MockIngress {
    pub fn new() -> MockIngress {
        MockIngress {
            inner: Arc::new(MockIngressInner {
                tunnels_connected: RwLock::new(Vec::new()),
            }),
        }
    }

}
