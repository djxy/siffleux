use std::sync::Arc;

use async_trait::async_trait;
use siffleux::{Error, HashedAuthKey, Ingress, IngressId, Tunnel};
use tokio::sync::broadcast::{self, Receiver, Sender};

#[derive(Clone)]
pub struct MockIngress {
    inner: Arc<MockIngressInner>,
}

struct MockIngressInner {
    id: IngressId,
    hashed_auth_key: HashedAuthKey,
    tunnel_sender: Sender<Tunnel>,
}

impl MockIngress {
    pub fn new(id: IngressId, hashed_auth_key: HashedAuthKey) -> Self {
        let (tunnel_sender, _) = broadcast::channel::<Tunnel>(8);
        Self {
            inner: Arc::new(MockIngressInner {
                id,
                hashed_auth_key,
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

    fn hashed_auth_key(&self) -> &HashedAuthKey {
        &self.inner.hashed_auth_key
    }

    fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
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
