use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use parking_lot::RwLock;
use tokio::{io::AsyncWriteExt, net::TcpSocket, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::{
    Egress, Error, IngressId, TunnelReadStream, TunnelStream, TunnelWriteStream,
    authentication::Authentication, client::egress::EgressId,
    protocols::v1::handle_protocol_v1_tcp_stream,
};

#[derive(Clone)]
pub struct TcpEgress {
    inner: Arc<TcpEgressInner>,
}

struct TcpEgressInner {
    id: EgressId,
    ingress_id: IngressId,
    authentication: Box<dyn Authentication>,
    target_addr: SocketAddr,
    cancellation_token: RwLock<Option<CancellationToken>>,
}

#[async_trait::async_trait]
impl Egress for TcpEgress {
    fn id(&self) -> &EgressId {
        &self.inner.id
    }

    fn ingress_id(&self) -> &IngressId {
        &self.inner.ingress_id
    }

    async fn start(&self) -> Result<(), Error> {
        let cancellation_token = {
            let mut cancellation_token = self.inner.cancellation_token.write();

            if cancellation_token.is_some() {
                return Err(Error::EgressAlreadyStarted);
            }

            let ct = CancellationToken::new();

            *cancellation_token = Some(ct.clone());

            ct
        };
        let mut tunnel = self.inner.authentication.connect(self).await?;
        let self_clone = self.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_stream_result = tunnel.accept_stream() => {
                        match accept_stream_result {
                            Ok((tunnel_read_stream, tunnel_write_stream, tunnel_stream)) => {
                                debug!(
                                    ingress_id = %self_clone.ingress_id(),
                                    egress_id = %self_clone.id(),
                                    tunnel_id = %tunnel.id(),
                                    "Received stream.",
                                );

                                self_clone.handle_stream(
                                    tunnel_stream,
                                    tunnel_read_stream,
                                    tunnel_write_stream,
                                    cancellation_token.clone()
                                );
                            }
                            Err(e) => {
                                if !matches!(e, Error::ClosedTunnel) {
                                    error!(egress_id = %self_clone.id(), "Error while accepting stream: {}", e);
                                }
                                break;
                            }
                        }
                    }
                    _ = tunnel.closed() => {
                        let mut retry: u32 = 0;

                        loop {
                            info!(egress_id = %self_clone.id(), "Reconnecting to server...");
                            match self_clone.inner.authentication.connect(&self_clone).await {
                                Ok(t) => {
                                    info!(egress_id = %self_clone.id(), "Reconnected to server.");
                                    tunnel = t;
                                    break;
                                }
                                Err(_) => {
                                    let duration = Duration::from_millis((100 * 2_u64.pow(retry)).min(30_000_u64));

                                    info!(egress_id = %self_clone.id(), "Failed to reconnect. Retry reconnecting in {:?}.", duration);

                                    tokio::select! {
                                        _ = sleep(duration) => {
                                            retry += 1;
                                        }
                                        _ = cancellation_token.cancelled() => {
                                            debug!(egress_id = %self_clone.id(), tunnel_id = %tunnel.id(), "Closing tunnel...");
                                            tunnel.close().await;
                                            debug!(egress_id = %self_clone.id(), tunnel_id = %tunnel.id(), "Tunnel closed.");

                                            info!(egress_id = %self_clone.id(), "Egress closed.");
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ = cancellation_token.cancelled() => {
                        debug!(egress_id = %self_clone.id(), tunnel_id = %tunnel.id(), "Closing tunnel...");
                        tunnel.close().await;
                        debug!(egress_id = %self_clone.id(), tunnel_id = %tunnel.id(), "Tunnel closed.");

                        info!(egress_id = %self_clone.id(), "Egress closed.");
                        return;
                    }
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        match self.inner.cancellation_token.write().take() {
            Some(cancellation_token) => {
                cancellation_token.cancel();

                Ok(())
            }
            None => Err(Error::EgressNotStarted),
        }
    }
}

impl TcpEgress {
    pub fn new(
        id: EgressId,
        authentication: Box<dyn Authentication>,
        ingress_id: IngressId,
        target_addr: SocketAddr,
    ) -> Self {
        Self {
            inner: Arc::new(TcpEgressInner {
                id,
                ingress_id,
                authentication,
                target_addr,
                cancellation_token: RwLock::new(None),
            }),
        }
    }

    fn handle_stream(
        &self,
        tunnel_stream: TunnelStream,
        tunnel_read_stream: TunnelReadStream,
        mut tunnel_write_stream: TunnelWriteStream,
        cancellation_token: CancellationToken,
    ) {
        let self_clone = self.clone();

        tokio::spawn(async move {
            let tcp_socket: TcpSocket = 'attempt: {
                for _ in 0..5 {
                    if let Ok(socket) = self_clone.get_tcp_socket().await {
                        break 'attempt socket;
                    }
                }

                let _ = tunnel_write_stream.shutdown().await;

                return;
            };

            let (tcp_remote_addr, (tcp_read_stream, tcp_write_stream)) =
                match tcp_socket.connect(self_clone.inner.target_addr).await {
                    Ok(tcp_stream) => {
                        tcp_stream.set_nodelay(true).unwrap();

                        (tcp_stream.peer_addr().unwrap(), tcp_stream.into_split())
                    }
                    Err(e) => {
                        error!(
                            "Error opening tcp connection to target={}: {e}",
                            self_clone.inner.target_addr
                        );
                        let _ = tunnel_write_stream.shutdown().await;

                        return;
                    }
                };

            handle_protocol_v1_tcp_stream(
                self_clone.ingress_id(),
                tunnel_stream,
                tunnel_read_stream,
                tunnel_write_stream,
                tcp_remote_addr,
                tcp_read_stream,
                tcp_write_stream,
                cancellation_token,
            )
            .await;
        });
    }

    async fn get_tcp_socket(&self) -> Result<TcpSocket, Error> {
        let local_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        let socket = TcpSocket::new_v4()?;

        socket.set_reuseaddr(true)?;
        socket.set_reuseport(true)?;
        socket.set_zero_linger()?;

        socket.bind(local_addr)?;

        Ok(socket)
    }
}
