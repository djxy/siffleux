use tokio::sync::watch;

use crate::{AuthKey, Error, IngressId, Tunnel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Stopped,
    Starting,
    Ready,
}

#[async_trait::async_trait]
pub trait Ingress: IngressClone + Send + Sync {
    fn id(&self) -> &IngressId;

    fn auth_key(&self) -> &AuthKey;

    fn state(&self) -> watch::Receiver<State>;

    fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error>;

    fn start(&self) -> Result<(), Error>;

    fn stop(&self) -> Result<(), Error>;
}

pub trait IngressClone {
    /// Create a clone of the ingress instance and returns it inside a Box.
    fn clone_box(&self) -> Box<dyn Ingress>;
}

impl<T: Ingress + Clone + 'static> IngressClone for T {
    fn clone_box(&self) -> Box<dyn Ingress> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Ingress> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}
