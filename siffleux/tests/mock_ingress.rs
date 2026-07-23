use std::sync::Arc;

use async_trait::async_trait;
use siffleux::{AuthKey, Error, Ingress, IngressId, Tunnel};
use tokio::sync::broadcast::{self, Receiver, Sender};

#[derive(Clone)]
pub struct MockIngress {
    inner: Arc<MockIngressInner>,
}

struct MockIngressInner {
    id: IngressId,
    auth_key: AuthKey,
    tunnel_sender: Sender<Tunnel>,
}

impl MockIngress {
    pub fn new(id: IngressId, auth_key: AuthKey) -> Self {
        let (tunnel_sender, _) = broadcast::channel::<Tunnel>(8);
        Self {
            inner: Arc::new(MockIngressInner {
                id,
                auth_key,
                tunnel_sender,
            }),
        }
    }

    pub fn subscribe_tunnel(&self) -> Receiver<Tunnel> {
        self.inner.tunnel_sender.subscribe()
    }
}

#[async_trait]
impl Ingress for MockIngress {
    fn id(&self) -> &IngressId {
        &self.inner.id
    }

    fn auth_key(&self) -> &AuthKey {
        &self.inner.auth_key
    }

    async fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
        self.inner.tunnel_sender.send(tunnel).unwrap();

        Ok(())
    }

    async fn start(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        Ok(())
    }
}
