use std::{collections::HashMap, sync::Arc};

use parking_lot::RwLock;
use tracing::{error, info};

use crate::{Egress, Error, client::egress::EgressId, common::ByteCounter};

#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

struct ClientInner {
    egress_by_id: RwLock<HashMap<EgressId, Box<dyn Egress>>>,
    byte_counter: ByteCounter,
}

impl Client {
    pub fn new() -> Self {
        Client {
            inner: Arc::new(ClientInner {
                egress_by_id: RwLock::new(HashMap::new()),
                byte_counter: ByteCounter::new(None),
            }),
        }
    }

    pub fn byte_counter(&self) -> &ByteCounter {
        &self.inner.byte_counter
    }

    pub fn assign_egress(&self, egress: Box<dyn Egress>) -> Result<(), Error> {
        let mut egress_by_id = self.inner.egress_by_id.write();

        if egress_by_id.contains_key(egress.id()) {
            return Err(Error::EgressIDAlreadyAssigned(egress.id().clone()));
        }

        egress_by_id.insert(egress.id().clone(), egress);

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Error> {
        info!("Closing egresses...");

        for egress in self.inner.egress_by_id.write().drain() {
            if let Err(e) = egress.1.stop().await {
                error!(
                    egress_id = %egress.0,
                    "Error while closing egress: {e}"
                );
            }
        }

        info!("Egresses closed.");

        Ok(())
    }
}
