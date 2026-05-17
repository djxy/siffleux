use crate::error::Error;
use crate::server::server_tunnel::ServerTunnel;

#[async_trait::async_trait]
pub trait Ingress: Send + Sync {
    fn on_tunnel_connected(&self, tunnel: ServerTunnel);

    async fn start(&self) -> Result<(), Error>;

    async fn stop(&self) -> Result<(), Error>;
}
