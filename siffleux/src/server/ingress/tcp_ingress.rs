use parking_lot::RwLock;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::sync::{Mutex, watch};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::common::AuthKey;
use crate::common::protocols::v1::handle_protocol_v1_tcp_stream;
use crate::{Error, State, Tunnel};
use crate::{Ingress, IngressId};

struct TcpIngressProcess {
    /// Token used to stop all the tasks related to the process.
    token: CancellationToken,
    /// The socket addr the TCP listener is bound to.
    local_addr: SocketAddr,
}

#[derive(Clone)]
pub struct TcpIngress {
    inner: Arc<TcpIngressInner>,
}

struct TcpIngressInner {
    id: IngressId,
    auth_key: AuthKey,
    listen_addr: SocketAddr,
    process_lock: Mutex<()>,
    process: RwLock<Option<Arc<TcpIngressProcess>>>,
    tunnels: RwLock<Vec<Tunnel>>,
    tunnel_rotation: AtomicUsize,
    state_sender: watch::Sender<State>,
    state_receiver: watch::Receiver<State>,
}

#[async_trait::async_trait]
impl Ingress for TcpIngress {
    fn id(&self) -> &IngressId {
        &self.inner.id
    }

    fn auth_key(&self) -> &AuthKey {
        &self.inner.auth_key
    }

    fn state(&self) -> watch::Receiver<State> {
        self.inner.state_receiver.clone()
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        self.inner.process.read().as_ref().map(|p| p.local_addr)
    }

    async fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
        if self.inner.process.read().as_ref().is_none() {
            return Err(Error::IngressNotStarted);
        }

        self.inner.tunnels.write().push(tunnel.clone());

        tokio::spawn(self.clone().handle_tunnel_close(tunnel.clone()));

        info!(
            ingress_id = %self.id(),
            tunnel_id = %&tunnel.id(),
            "Assigned tunnel to TCP ingress."
        );

        Ok(())
    }

    async fn start(&self) -> Result<(), Error> {
        let _lock = self.inner.process_lock.lock().await;

        if self.inner.process.read().is_some() {
            return Err(Error::IngressAlreadyStarted);
        }

        let tcp_listener = self.create_tcp_listener()?;
        let process = Arc::new(TcpIngressProcess {
            local_addr: tcp_listener.local_addr()?,
            token: CancellationToken::new(),
        });

        info!(ingress_id = %self.id(), "Starting TCP ingress");

        {
            let mut process_guard = self.inner.process.write();
            *process_guard = Some(process.clone());
        }

        tokio::spawn(self.clone().start_socket_to_tunnel(process, tcp_listener));

        self.state()
            .wait_for(|state| *state == State::Started)
            .await?;

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        if let Some(process) = self.inner.process.read().as_ref() {
            info!(egress_id = %self.id(), "Stopping UDP ingress");
            process.token.cancel();
        } else {
            return Err(Error::IngressNotStarted);
        }

        self.state()
            .wait_for(|state| *state == State::Stopped)
            .await?;

        Ok(())
    }
}

impl TcpIngress {
    pub fn new(id: IngressId, auth_key: AuthKey, socket_addr: SocketAddr) -> Self {
        let (state_sender, state_receiver) = watch::channel(State::Stopped);

        Self {
            inner: Arc::new(TcpIngressInner {
                id,
                auth_key,
                listen_addr: socket_addr,
                process_lock: Mutex::new(()),
                process: RwLock::new(None),
                tunnels: RwLock::new(Vec::new()),
                tunnel_rotation: AtomicUsize::new(0),
                state_sender,
                state_receiver,
            }),
        }
    }

    async fn start_socket_to_tunnel(
        self,
        process: Arc<TcpIngressProcess>,
        tcp_listener: TcpListener,
    ) {
        info!(ingress_id = %self.id(), "Ready to accept TCP connections on {}.", self.inner.listen_addr);

        let _ = self.inner.state_sender.send(State::Started);

        loop {
            tokio::select! {
                result = tcp_listener.accept() => {
                    match result {
                        Ok((tcp_stream, _)) => {
                            info!(
                                ingress_id = %self.id(),
                                remote_addr = %tcp_stream.peer_addr().unwrap(),
                                "Received TCP connection"
                            );

                            tokio::spawn(self.clone().start_stream_to_tunnel(process.clone(), tcp_stream));
                        }
                        Err(e) => {
                            error!(ingress_id = %self.id(), "Error accepting TCP stream: {e}");
                            break;
                        }
                    }
                }
                _ = process.token.cancelled() => {
                    debug!(ingress_id = %self.id(), "Stopped socket => tunnel.");
                    break;
                }
            }
        }

        self.inner.process.write().take();
        process.token.cancel();

        let _ = self.inner.state_sender.send(State::Stopped);

        info!(egress_id = %self.id(), "Stopped UDP ingress");
    }

    async fn start_stream_to_tunnel(self, process: Arc<TcpIngressProcess>, tcp_stream: TcpStream) {
        let Some(tunnel) = self.get_tunnel_to_connect() else {
            warn!(egress_id = %self.id(), "No tunnel connected.");
            return;
        };

        tcp_stream.set_nodelay(true).unwrap();

        let tcp_remote_addr = tcp_stream.peer_addr().unwrap();
        let (tcp_read_stream, tcp_write_stream): (OwnedReadHalf, OwnedWriteHalf) =
            tcp_stream.into_split();

        match tunnel.create_stream().await {
            Ok((tunnel_read_stream, tunnel_write_stream, tunnel_stream)) => {
                handle_protocol_v1_tcp_stream(
                    self.id(),
                    tunnel_stream,
                    tunnel_read_stream,
                    tunnel_write_stream,
                    tcp_remote_addr,
                    tcp_read_stream,
                    tcp_write_stream,
                    process.token.clone(),
                )
                .await;
            }
            Err(e) => {
                error!(egress_id = %self.id(), "Error creating tunnel stream: {e}");
            }
        }
    }

    fn get_tunnel_to_connect(&self) -> Option<Tunnel> {
        let tunnels = self.inner.tunnels.read();

        if tunnels.is_empty() {
            return None;
        }

        if tunnels.len() == 1 {
            return Some(tunnels[0].clone());
        }

        Some(
            tunnels[self.inner.tunnel_rotation.fetch_add(1, Ordering::Relaxed) % tunnels.len()]
                .clone(),
        )
    }

    fn create_tcp_listener(&self) -> Result<TcpListener, Error> {
        let socket = TcpSocket::new_v4()?;

        socket.set_reuseaddr(true)?;
        socket.set_reuseport(true)?;
        socket.set_zero_linger()?;

        let buffer_size = 16 * 1024 * 1024; // 16mb

        socket.set_recv_buffer_size(buffer_size)?;
        socket.set_send_buffer_size(buffer_size)?;

        socket.bind(self.inner.listen_addr)?;

        Ok(socket.listen(1024)?)
    }

    async fn handle_tunnel_close(self, tunnel: Tunnel) {
        tunnel.closed().await;

        let mut tunnels = self.inner.tunnels.write();

        if let Some(i) = tunnels.iter().position(|t| t.id() == tunnel.id()) {
            tunnels.swap_remove(i);
        }
    }
}
