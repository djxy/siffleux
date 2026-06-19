use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tracing::{debug, error};

use crate::{Egress, Error, Tunnel};

#[derive(Clone)]
pub struct EndEgress {
    inner: Arc<EndEgressInner>,
}

struct EndEgressInner {
    tunnel: Tunnel,
}

#[async_trait::async_trait]
impl Egress for EndEgress {
    async fn start(&self) -> Result<(), Error> {
        let self_clone = self.clone();

        tokio::spawn(async move {
            while let Ok((_, mut tunnel_write_stream, stream)) =
                self_clone.inner.tunnel.accept_stream().await
            {
                let tunnel_id = self_clone.inner.tunnel.id().clone();
                let ingress_id = self_clone.inner.tunnel.ingress_id().clone();

                tokio::spawn(async move {
                    debug!(
                        "Received tunnel stream={} from tunnel_id={} ingress_id={}",
                        stream.id(),
                        tunnel_id,
                        ingress_id
                    );

                    tunnel_write_stream.shutdown().await.unwrap();

                    debug!(
                        "Closed tunnel stream={} from tunnel_id={} ingress_id={}",
                        stream.id(),
                        tunnel_id,
                        ingress_id
                    );
                });
            }

            if let Err(e) = self_clone.stop().await {
                error!("Error while stopping end egress: {}", e)
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        Ok(())
    }
}

impl EndEgress {
    pub fn new(tunnel: Tunnel) -> Self {
        Self {
            inner: Arc::new(EndEgressInner { tunnel }),
        }
    }
}
