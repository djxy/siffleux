use std::{net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use siffleux::{AuthKey, Error, Ingress, IngressId, State, Tunnel};
use tokio::sync::{
    broadcast::{self, Receiver, Sender},
    watch,
};

#[derive(Clone)]
pub struct MockIngress {
    inner: Arc<MockIngressInner>,
}

struct MockIngressInner {
    id: IngressId,
    auth_key: AuthKey,
    tunnel_sender: Sender<Tunnel>,
    _state_sender: watch::Sender<State>,
    state_receiver: watch::Receiver<State>,
}

impl MockIngress {
    pub fn new(id: IngressId, auth_key: AuthKey) -> Self {
        let (tunnel_sender, _) = broadcast::channel::<Tunnel>(8);
        let (_state_sender, state_receiver) = watch::channel(State::Stopped);

        Self {
            inner: Arc::new(MockIngressInner {
                id,
                auth_key,
                tunnel_sender,
                _state_sender,
                state_receiver,
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

    fn state(&self) -> watch::Receiver<State> {
        self.inner.state_receiver.clone()
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        None
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
