use tokio_util::sync::CancellationToken;

use crate::{Error, IngressId, client::egress::EgressId};

#[async_trait::async_trait]
pub trait Egress: EgressClone + Send + Sync {
    fn id(&self) -> &EgressId;

    fn ingress_id(&self) -> &IngressId;

    async fn start(&self) -> Result<(), Error>;

    async fn stop(&self) -> Result<(), Error>;

    /// Returns true if is running or false if stopped.
    fn is_running(&self) -> bool;

    /// If the egress is stopped, it will return None. If the egress is running, it will return the CancellationToken
    /// related to the current execution. If the token is cancelled, it means the egress is stopped.
    fn stopped(&self) -> Option<CancellationToken>;
}

pub trait EgressClone {
    /// Create a clone of the egress instance and returns it inside a Box.
    fn clone_box(&self) -> Box<dyn Egress>;
}

impl<T: Egress + Clone + 'static> EgressClone for T {
    fn clone_box(&self) -> Box<dyn Egress> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Egress> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}
