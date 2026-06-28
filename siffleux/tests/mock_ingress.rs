use std::sync::Arc;

use async_trait::async_trait;
use siffleux::{Error, HashedAuthKey, Ingress, IngressId, Tunnel};
use tokio::sync::watch::{self, Receiver, Sender};

#[derive(Clone)]
pub struct MockIngress {
    inner: Arc<MockIngressInner>,
}

struct MockIngressInner {
    id: IngressId,
    hashed_auth_key: HashedAuthKey,
    tunnel_sender: Sender<Option<Tunnel>>,
    _tunnel_receiver: Receiver<Option<Tunnel>>,
}

impl MockIngress {
    pub fn new(id: IngressId, hashed_auth_key: HashedAuthKey) -> Self {
        let (tunnel_sender, _tunnel_receiver) = watch::channel::<Option<Tunnel>>(None);
        Self {
            inner: Arc::new(MockIngressInner {
                id,
                hashed_auth_key,
                tunnel_sender,
                _tunnel_receiver,
            }),
        }
    }

    pub async fn accept_tunnel(&self) -> Tunnel {
        self.inner
            .tunnel_sender
            .subscribe()
            .wait_for(|t| t.is_some())
            .await
            .unwrap()
            .clone()
            .unwrap()
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
        self.inner.tunnel_sender.send(Some(tunnel)).unwrap();

        Ok(())
    }

    async fn start(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        Ok(())
    }
}
