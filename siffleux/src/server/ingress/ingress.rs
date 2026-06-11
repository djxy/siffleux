use crate::{
    Error, Tunnel,
    common::{HashedAuthKey, IngressId},
};

#[async_trait::async_trait]
pub trait Ingress: IngressClone + Send + Sync {
    fn id(&self) -> &IngressId;

    fn hashed_auth_key(&self) -> &HashedAuthKey;

    fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error>;

    async fn start(&self) -> Result<(), Error>;

    async fn stop(&self) -> Result<(), Error>;
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
