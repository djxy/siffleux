use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use bytes::Bytes;
use parking_lot::RwLock;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::{
    net::UdpSocket,
    sync::{
        Mutex,
        mpsc::{self, Receiver, UnboundedSender},
        watch::{self},
    },
    task::JoinHandle,
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::{
    Egress, Error, IngressId, Tunnel,
    authentication::Authentication,
    client::egress::{EgressId, egress::State},
    frames::v1::{extract_origin_socket_addr_from_datagram, to_datagram},
};

const MAX_MESSAGES_RECEIVED: usize = 128;

struct UdpEgressProcess {
    /// Token used to stop all the tasks related to the process.
    token: CancellationToken,
    tunnel_task: JoinHandle<()>,
}

#[derive(Clone)]
pub struct UdpEgress {
    inner: Arc<UdpEgressInner>,
}

struct UdpEgressInner {
    id: EgressId,
    ingress_id: IngressId,
    authentication: Box<dyn Authentication>,
    target_addr: SocketAddr,
    process: RwLock<Option<UdpEgressProcess>>,
    process_lock: Mutex<()>,
    state_sender: watch::Sender<State>,
    state_receiver: watch::Receiver<State>,
}

#[async_trait::async_trait]
impl Egress for UdpEgress {
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

        let _ = self.inner.state_sender.send(State::Starting);

        let mut process = self.inner.process.write();
        let token = CancellationToken::new();

        *process = Some(UdpEgressProcess {
            tunnel_task: self.start_tunnel(token.clone()),
            token,
        });

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

impl UdpEgress {
    pub fn new(
        id: EgressId,
        authentication: Box<dyn Authentication>,
        ingress_id: IngressId,
        target_addr: SocketAddr,
    ) -> Self {
        let (state_sender, state_receiver) = watch::channel(State::Stopped);

        Self {
            inner: Arc::new(UdpEgressInner {
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

                        self_clone.start_tunnel_to_socket(tunnel.clone(), process_token.clone());

                        tokio::select! {
                            _ = process_token.cancelled() => {}
                            _ = tunnel.closed() => {}
                        }

                        if process_token.is_cancelled() {
                            tunnel.close().await;
                            return;
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

    fn start_tunnel_to_socket(
        &self,
        tunnel: Tunnel,
        process_token: CancellationToken,
    ) -> JoinHandle<()> {
        let self_clone = self.clone();

        tokio::spawn(async move {
            let (expired_origin_socket_addr_sender, mut expired_origin_socket_addr_receiver) =
                mpsc::unbounded_channel::<SocketAddr>();
            let mut udp_sockets: HashMap<SocketAddr, mpsc::Sender<Bytes>> =
                HashMap::with_capacity(64);

            info!(egress_id = %self_clone.id(), "Started");
            let _ = self_clone.inner.state_sender.send(State::Ready);

            loop {
                tokio::select! {
                    datagram_received_result = tunnel.read_datagram() => {
                        match datagram_received_result {
                            Ok(bytes) => {
                                self_clone.process_datagram(
                                    bytes,
                                    &tunnel,
                                    &mut udp_sockets,
                                    &expired_origin_socket_addr_sender,
                                    &process_token
                                );
                            }
                            Err(e) => {
                                if !matches!(e, Error::ClosedTunnel) {
                                    error!(egress_id = %self_clone.id(), "Error while receiving a datagram from tunnel: {}", e);
                                }
                            }
                        }
                    }
                    socket_addr = expired_origin_socket_addr_receiver.recv() => {
                        if let Some(socket_addr) = socket_addr {
                            udp_sockets.remove(&socket_addr);
                        }
                    }
                    _ = tunnel.closed() => {
                        return;
                    }
                    _ = process_token.cancelled() => {
                        return;
                    }
                }
            }
        })
    }

    fn process_datagram(
        &self,
        mut bytes: Bytes,
        tunnel: &Tunnel,
        udp_sockets: &mut HashMap<SocketAddr, mpsc::Sender<Bytes>>,
        expired_origin_socket_addr_sender: &UnboundedSender<SocketAddr>,
        process_token: &CancellationToken,
    ) {
        let Some(origin_socket_addr) = extract_origin_socket_addr_from_datagram(&mut bytes) else {
            return;
        };

        if let Some(tunnel_to_socket_sender) = udp_sockets.get(&origin_socket_addr) {
            let _ = tunnel_to_socket_sender.try_send(bytes);
        } else {
            let (tunnel_to_socket_sender, tunnel_to_socket_receiver) =
                mpsc::channel::<Bytes>(MAX_MESSAGES_RECEIVED);

            self.pipe_tunnel_to_socket_datagram(
                tunnel.clone(),
                origin_socket_addr,
                tunnel_to_socket_receiver,
                process_token.clone(),
                expired_origin_socket_addr_sender.clone(),
            );

            let _ = tunnel_to_socket_sender.try_send(bytes);

            udp_sockets.insert(origin_socket_addr, tunnel_to_socket_sender);
        }
    }

    fn pipe_tunnel_to_socket_datagram(
        &self,
        tunnel: Tunnel,
        origin_socket_addr: SocketAddr,
        mut tunnel_to_socket_receiver: Receiver<Bytes>,
        process_token: CancellationToken,
        expired_origin_socket_addr_sender: UnboundedSender<SocketAddr>,
    ) {
        let self_clone = self.clone();

        tokio::spawn(async move {
            let Ok(udp_socket) = self_clone.get_udp_socket().await else {
                let _ = expired_origin_socket_addr_sender.send(origin_socket_addr);
                return;
            };

            let udp_socket = Arc::from(udp_socket);
            let socket_token = CancellationToken::new();

            self_clone.pipe_socket_to_tunnel_datagram(
                tunnel.clone(),
                origin_socket_addr,
                udp_socket.clone(),
                process_token.clone(),
                socket_token.clone(),
            );

            let mut bytes_received: Vec<Bytes> = Vec::with_capacity(MAX_MESSAGES_RECEIVED);

            loop {
                tokio::select! {
                    count = tunnel_to_socket_receiver.recv_many(&mut bytes_received, MAX_MESSAGES_RECEIVED) => {
                        if count == 0 {
                            return;
                        }

                        for bytes in bytes_received.drain(..) {
                            let _ = udp_socket.try_send(&bytes);
                        }
                    }
                    _ = sleep(Duration::from_secs(60)) => {
                        let _ = expired_origin_socket_addr_sender.send(origin_socket_addr);
                        socket_token.cancel();
                        return;
                    }
                    _ = process_token.cancelled() => {
                        return;
                    }
                    _ = tunnel.closed() => {
                        let _ = expired_origin_socket_addr_sender.send(origin_socket_addr);
                        return;
                    }
                }
            }
        });
    }

    fn pipe_socket_to_tunnel_datagram(
        &self,
        tunnel: Tunnel,
        origin_socket_addr: SocketAddr,
        udp_socket: Arc<UdpSocket>,
        process_token: CancellationToken,
        socket_token: CancellationToken,
    ) {
        tokio::spawn(async move {
            let mut buffer = [0u8; 1500];

            loop {
                tokio::select! {
                    recv_result = udp_socket.recv(&mut buffer) => {
                        match recv_result {
                            Ok(size) => {
                                let _ = tunnel.send_datagram(to_datagram(origin_socket_addr, &buffer, size));
                            }
                            Err(_) => {}
                        }
                    }
                    _ = socket_token.cancelled() => {
                        return;
                    }
                    _ = tunnel.closed() => {
                        return;
                    }
                    _ = process_token.cancelled() => {
                        return;
                    }
                }
            }
        });
    }

    async fn get_udp_socket(&self) -> Result<UdpSocket, Error> {
        let local_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

        #[cfg(unix)]
        socket.set_reuse_port(true)?;
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;

        let buffer_size = 4 * 1024 * 1024; // 4mb

        socket.set_recv_buffer_size(buffer_size)?;
        socket.set_send_buffer_size(buffer_size)?;

        socket.bind(&local_addr.into())?;

        let udp_socket = UdpSocket::from_std(socket.into())?;

        udp_socket.connect(self.inner.target_addr).await?;
        udp_socket.writable().await?;

        Ok(udp_socket)
    }
}
