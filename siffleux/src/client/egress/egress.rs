use crate::{Error, IngressId, client::egress::EgressId};

#[async_trait::async_trait]
pub trait Egress: EgressClone + Send + Sync {
    fn id(&self) -> &EgressId;

    fn ingress_id(&self) -> &IngressId;

    async fn start(&self) -> Result<(), Error>;

    async fn stop(&self) -> Result<(), Error>;
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
