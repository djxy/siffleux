use std::{net::SocketAddr, sync::Arc};

use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::{
    Error, Tunnel,
    client::egress::Egress,
    common::{
        protocols::v1::handle_protocol_v1_tcp_stream,
        tunnel::{TunnelReadStream, TunnelWriteStream},
    },
};

#[derive(Clone)]
pub struct TcpEgress {
    inner: Arc<TcpEgressInner>,
}

struct TcpEgressInner {
    tunnel: Tunnel,
    target_addr: SocketAddr,
    cancellation_token: CancellationToken,
}

#[async_trait::async_trait]
impl Egress for TcpEgress {
    async fn start(&self) -> Result<(), Error> {
        let self_clone = self.clone();

        tokio::spawn(async move {
            while let Ok((tunnel_read_stream, tunnel_write_stream, _)) =
                self_clone.inner.tunnel.accept_stream().await
            {
                info!(
                    "Received tcp connection from tunnel_id={} ingress_id={} on tcp_egress={}",
                    self_clone.inner.tunnel.id(),
                    self_clone.inner.tunnel.ingress_id(),
                    self_clone.inner.target_addr
                );

                self_clone.handle_stream(tunnel_read_stream, tunnel_write_stream);
            }

            if let Err(e) = self_clone.stop().await {
                error!("Error while stopping tcp egress: {}", e)
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        self.inner.cancellation_token.cancel();

        Ok(())
    }
}

impl TcpEgress {
    pub fn new(tunnel: Tunnel, target_addr: SocketAddr) -> Self {
        Self {
            inner: Arc::new(TcpEgressInner {
                tunnel,
                target_addr,
                cancellation_token: CancellationToken::new(),
            }),
        }
    }

    fn handle_stream(
        &self,
        tunnel_read_stream: TunnelReadStream,
        tunnel_write_stream: TunnelWriteStream,
    ) {
        let self_clone = self.clone();

        tokio::spawn(async move {
            let (tcp_read_stream, tcp_write_stream) =
                match TcpStream::connect(self_clone.inner.target_addr).await {
                    Ok(tcp_stream) => tcp_stream.into_split(),
                    Err(e) => {
                        warn!(
                            "Error opening tcp connection to target={}: {e}",
                            self_clone.inner.target_addr
                        );
                        return;
                    }
                };

            handle_protocol_v1_tcp_stream(
                tunnel_read_stream,
                tunnel_write_stream,
                tcp_read_stream,
                tcp_write_stream,
                self_clone.inner.cancellation_token.clone(),
            );
        });
    }
}
