use crate::{Error, IngressId, Tunnel};

#[async_trait::async_trait]
pub trait Ingress: Send + Sync {
    fn id(&self) -> &IngressId;

    fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error>;

    async fn start(&self) -> Result<(), Error>;

    async fn stop(&self) -> Result<(), Error>;
}
