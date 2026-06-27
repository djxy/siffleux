use crate::{Egress, Error, Tunnel};

#[async_trait::async_trait]
pub trait Authentication: Send + Sync {
    async fn connect(&self, egress: &dyn Egress) -> Result<Tunnel, Error>;
}
