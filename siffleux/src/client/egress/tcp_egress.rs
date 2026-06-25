use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{
        Arc,
        atomic::{AtomicU16, Ordering},
    },
};

use tokio::{io::AsyncWriteExt, net::TcpSocket};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use crate::{
    Egress, Error, Tunnel, TunnelReadStream, TunnelStream, TunnelWriteStream,
    protocols::v1::handle_protocol_v1_tcp_stream,
};

#[derive(Clone)]
pub struct TcpEgress {
    inner: Arc<TcpEgressInner>,
}

struct TcpEgressInner {
    tunnel: Tunnel,
    target_addr: SocketAddr,
    cancellation_token: CancellationToken,
    port_pool: AtomicU16,
}

#[async_trait::async_trait]
impl Egress for TcpEgress {
    async fn start(&self) -> Result<(), Error> {
        let self_clone = self.clone();

        tokio::spawn(async move {
            while let Ok((tunnel_read_stream, tunnel_write_stream, tunnel_stream)) =
                self_clone.inner.tunnel.accept_stream().await
            {
                debug!(
                    "Received tunnel stream from tunnel_id={} ingress_id={} on tcp_egress={}",
                    self_clone.inner.tunnel.server_id(),
                    self_clone.inner.tunnel.ingress_id(),
                    self_clone.inner.target_addr
                );

                self_clone.handle_stream(tunnel_stream, tunnel_read_stream, tunnel_write_stream);
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
                port_pool: AtomicU16::new(0),
            }),
        }
    }

    fn handle_stream(
        &self,
        tunnel_stream: TunnelStream,
        tunnel_read_stream: TunnelReadStream,
        mut tunnel_write_stream: TunnelWriteStream,
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
                self_clone.inner.tunnel.ingress_id(),
                tunnel_stream,
                tunnel_read_stream,
                tunnel_write_stream,
                tcp_remote_addr,
                tcp_read_stream,
                tcp_write_stream,
                self_clone.inner.cancellation_token.clone(),
            )
            .await;
        });
    }

    async fn get_tcp_socket(&self) -> Result<TcpSocket, Error> {
        let local_addr = SocketAddr::new(
            Ipv4Addr::UNSPECIFIED.into(),
            10000 + (self.inner.port_pool.fetch_add(1, Ordering::SeqCst) % 40000),
        );

        let socket = TcpSocket::new_v4()?;

        socket.set_reuseaddr(true)?;
        socket.set_reuseport(true)?;
        socket.set_zero_linger()?;

        socket.bind(local_addr)?;

        Ok(socket)
    }
}
