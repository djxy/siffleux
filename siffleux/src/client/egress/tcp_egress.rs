use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use parking_lot::RwLock;
use tokio::{
    io::AsyncWriteExt,
    net::TcpSocket,
    sync::{Mutex, watch},
    task::JoinHandle,
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::{
    Egress, Error, IngressId, State, TunnelReadStream, TunnelStream, TunnelWriteStream,
    authentication::Authentication, client::egress::EgressId,
    protocols::v1::handle_protocol_v1_tcp_stream,
};

struct TcpEgressProcess {
    /// Token used to stop all the tasks related to the process.
    token: CancellationToken,
    tunnel_task: JoinHandle<()>,
}

#[derive(Clone)]
pub struct TcpEgress {
    inner: Arc<TcpEgressInner>,
}

struct TcpEgressInner {
    id: EgressId,
    ingress_id: IngressId,
    authentication: Box<dyn Authentication>,
    target_addr: SocketAddr,
    process: RwLock<Option<TcpEgressProcess>>,
    process_lock: Mutex<()>,
    state_sender: watch::Sender<State>,
    state_receiver: watch::Receiver<State>,
}

#[async_trait::async_trait]
impl Egress for TcpEgress {
    fn id(&self) -> &EgressId {
        &self.inner.id
    }

    fn ingress_id(&self) -> &IngressId {
        &self.inner.ingress_id
    }

    fn state(&self) -> watch::Receiver<State> {
        self.inner.state_receiver.clone()
    }

    async fn start(&self) -> Result<(), Error> {
        let _lock = self.inner.process_lock.lock().await;

        if self.inner.process.read().is_some() {
            return Err(Error::EgressAlreadyStarted);
        }

        info!(egress_id = %self.id(), "Starting");

        let mut process = self.inner.process.write();
        let token = CancellationToken::new();

        *process = Some(TcpEgressProcess {
            tunnel_task: self.start_tunnel(token.clone()),
            token,
        });

        info!(egress_id = %self.id(), "Started");

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        let process = self.inner.process.write().take();

        match process {
            Some(process) => {
                info!(egress_id = %self.id(), "Stopping");
                process.token.cancel();
                process.tunnel_task.await?;
                let _ = self.inner.state_sender.send(State::Stopped);
                info!(egress_id = %self.id(), "Stopped");

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
        let (state_sender, state_receiver) = watch::channel(State::Stopped);

        Self {
            inner: Arc::new(TcpEgressInner {
                id,
                ingress_id,
                authentication,
                target_addr,
                process: RwLock::new(None),
                process_lock: Mutex::new(()),
                state_sender,
                state_receiver,
            }),
        }
    }

    fn start_tunnel(&self, process_token: CancellationToken) -> JoinHandle<()> {
        let self_clone = self.clone();

        tokio::spawn(async move {
            let mut retry: u32 = 0;

            loop {
                info!(egress_id = %self_clone.id(), "Connecting to server...");

                match self_clone.inner.authentication.connect(&self_clone).await {
                    Ok(tunnel) => {
                        retry = 0;
                        info!(egress_id = %self_clone.id(), "Tunnel established.");
                        info!(egress_id = %self_clone.id(), "TCP egress started.");
                        let _ = self_clone.inner.state_sender.send(State::Started);

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
                                                process_token.clone()
                                            );
                                        }
                                        Err(e) => {
                                            if !matches!(e, Error::ClosedTunnel) {
                                                error!(egress_id = %self_clone.id(), "Error while accepting stream: {}", e);
                                            }
                                        }
                                    }
                                }
                                _ = tunnel.closed() => {
                                    warn!(egress_id = %self_clone.id(), "Tunnel disconnected.");
                                    break;
                                }
                                _ = process_token.cancelled() => {
                                    tunnel.close().await;
                                    return;
                                }
                            }
                        }
                    }
                    Err(_) => {
                        let duration =
                            Duration::from_millis((100 * 2_u64.pow(retry)).min(30_000_u64));

                        info!(egress_id = %self_clone.id(), "Failed to connect. Reconnecting in {:?}.", duration);

                        tokio::select! {
                            _ = sleep(duration) => {
                                retry += 1;
                            }
                            _ = process_token.cancelled() => {
                                debug!(egress_id = %self_clone.id(), "Cancelling reconnection to the server.");
                                return;
                            }
                        }
                    }
                }
            }
        })
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
                for _ in 0..3 {
                    if let Ok(socket) = self_clone.get_tcp_socket() {
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

    fn get_tcp_socket(&self) -> Result<TcpSocket, Error> {
        let local_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        let socket = TcpSocket::new_v4()?;

        socket.set_reuseaddr(true)?;
        socket.set_reuseport(true)?;
        socket.set_zero_linger()?;

        let buffer_size = 16 * 1024 * 1024; // 16mb

        socket.set_recv_buffer_size(buffer_size)?;
        socket.set_send_buffer_size(buffer_size)?;

        socket.bind(local_addr)?;

        Ok(socket)
    }
}
