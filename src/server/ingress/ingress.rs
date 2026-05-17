use crate::error::Error;
use crate::server::Server;

#[async_trait::async_trait]
pub trait Ingress: Send + Sync {
    async fn start(&self, server: &Server) -> Result<(), Error>;

    async fn stop(&self, server: &Server) -> Result<(), Error>;
}
