use crate::common::error::Error;
use crate::common::tunnel::Tunnel;
use crate::common::types::IngressId;
use crate::server::server::Server;

#[async_trait::async_trait]
pub trait Ingress: Send + Sync {
    fn id(&self) -> &IngressId;

    fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error>;

    async fn start(&self, server: &Server) -> Result<(), Error>;

    async fn stop(&self, server: &Server) -> Result<(), Error>;
}
