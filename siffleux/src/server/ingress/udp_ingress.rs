use async_trait::async_trait;
use parking_lot::RwLock;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::UdpSocket;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::common::AuthKey;
use crate::frames::v1::{extract_origin_socket_addr_from_datagram, to_datagram};
use crate::server::State;
use crate::{Error, Tunnel};
use crate::{Ingress, IngressId};

struct UdpIngressProcess {
    /// Token used to stop all the tasks related to the process.
    token: CancellationToken,
    udp_socket: UdpSocket,
}

#[derive(Clone)]
pub struct UdpIngress {
    inner: Arc<UdpIngressInner>,
}

struct UdpIngressInner {
    id: IngressId,
    auth_key: AuthKey,
    listen_addr: SocketAddr,
    process: RwLock<Option<Arc<UdpIngressProcess>>>,
    tunnels: RwLock<Vec<Tunnel>>,
    tunnel_rotation: AtomicUsize,
    state_sender: watch::Sender<State>,
    state_receiver: watch::Receiver<State>,
}

#[async_trait]
impl Ingress for UdpIngress {
    fn id(&self) -> &IngressId {
        &self.inner.id
    }

    fn auth_key(&self) -> &AuthKey {
        &self.inner.auth_key
    }

    fn state(&self) -> watch::Receiver<State> {
        self.inner.state_receiver.clone()
    }

    fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
        let Some(process) = self.inner.process.read().as_ref().cloned() else {
            return Err(Error::IngressNotStarted);
        };

        self.inner.tunnels.write().push(tunnel.clone());

        tokio::spawn(self.clone().start_tunnel_to_socket(process, tunnel.clone()));
        tokio::spawn(self.clone().handle_tunnel_close(tunnel.clone()));

        info!(
            ingress_id = %self.id(),
            tunnel_id = %&tunnel.id(),
            "Assigned tunnel to UDP ingress."
        );

        Ok(())
    }

    fn start(&self) -> Result<(), Error> {
        let mut process_guard = self.inner.process.write();

        if process_guard.is_some() {
            return Err(Error::IngressAlreadyStarted);
        }

        let token = CancellationToken::new();
        let udp_socket = self.create_udp_socket()?;
        let process = Arc::new(UdpIngressProcess {
            token: token.clone(),
            udp_socket: udp_socket,
        });

        tokio::spawn(self.clone().start_socket_to_tunnel(process.clone()));

        *process_guard = Some(process);

        Ok(())
    }

    fn stop(&self) -> Result<(), Error> {
        let process = self.inner.process.write().take();

        match process {
            Some(process) => {
                info!(egress_id = %self.id(), "Stopping UDP ingress");
                process.token.cancel();
                Ok(())
            }
            None => Err(Error::EgressNotStarted),
        }
    }
}

impl UdpIngress {
    pub fn new(id: IngressId, auth_key: AuthKey, listen_addr: SocketAddr) -> Self {
        let (state_sender, state_receiver) = watch::channel(State::Stopped);

        Self {
            inner: Arc::new(UdpIngressInner {
                id,
                auth_key,
                listen_addr,
                process: RwLock::new(None),
                tunnels: RwLock::new(Vec::new()),
                tunnel_rotation: AtomicUsize::new(0),
                state_sender,
                state_receiver,
            }),
        }
    }

    pub fn get_socket_addr(&self) -> Option<SocketAddr> {
        self.inner
            .process
            .read()
            .as_ref()
            .map(|p| p.udp_socket.local_addr().unwrap().clone())
    }

    async fn start_socket_to_tunnel(self, process: Arc<UdpIngressProcess>) {
        info!(ingress_id = %self.id(), "Starting UDP ingress");

        let _ = self.inner.state_sender.send(State::Starting);

        if let Err(e) = process.udp_socket.connect(self.inner.listen_addr).await {
            let _ = self.inner.state_sender.send(State::Stopped);

            error!(egress_id = %self.id(), "Failed to start UDP ingress: {e}");
        }

        if let Err(e) = process.udp_socket.writable().await {
            let _ = self.inner.state_sender.send(State::Stopped);

            error!(egress_id = %self.id(), "Failed to start UDP ingress: {e}");
        }

        info!(ingress_id = %self.id(), "Ready to receive UDP datagrams on {}.", process.udp_socket.local_addr().unwrap());

        let _ = self.inner.state_sender.send(State::Ready);

        let mut buffer = [0u8; 1500];

        loop {
            tokio::select! {
                result = process.udp_socket.recv_from(&mut buffer) => {
                    match result {
                        Ok((len, socket_addr)) => {
                            if let Err(e) = self.send_datagram_to_tunnel(socket_addr, &buffer, len).await {
                                error!(ingress_id = %self.id(), "Error while sending datagram to tunnel: {e}");
                            } else {
                                info!("{len} bytes sent");
                            }
                        }
                        Err(e) => {
                            error!(ingress_id = %self.id(), "Error while receiving datagram from socket: {e}");
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

        let _ = self.inner.state_sender.send(State::Stopped);

        info!(egress_id = %self.id(), "Stopped UDP ingress");
    }

    async fn send_datagram_to_tunnel(
        &self,
        socket_addr: SocketAddr,
        data: &[u8],
        data_size: usize,
    ) -> Result<(), Error> {
        let tunnel = {
            let tunnels = self.inner.tunnels.read();

            if tunnels.is_empty() {
                return Err(Error::NoTunnelAvailable);
            }

            if tunnels.len() == 1 {
                tunnels[0].clone()
            } else {
                tunnels[self.inner.tunnel_rotation.fetch_add(1, Ordering::Relaxed) % tunnels.len()]
                    .clone()
            }
        };

        if let Err(_) = tunnel.try_send_datagram(to_datagram(socket_addr, data, data_size)) {
            tunnel
                .send_datagram(to_datagram(socket_addr, data, data_size))
                .await?
        }

        Ok(())
    }

    async fn start_tunnel_to_socket(self, process: Arc<UdpIngressProcess>, tunnel: Tunnel) {
        loop {
            tokio::select! {
                bytes_result = tunnel.read_datagram() => {
                    match bytes_result {
                        Ok(mut bytes) => {
                            if let Some(origin_socket_addr) = extract_origin_socket_addr_from_datagram(&mut bytes) {
                                let _ = process.udp_socket.send_to(&bytes[..], origin_socket_addr).await;
                            };
                        }
                        Err(e) => {
                            if matches!(e, Error::ClosedTunnel) {
                                warn!(
                                    ingress_id = %self.id(),
                                    tunnel_id = %&tunnel.id(),
                                    "Tunnel closed. Stopping receiving from it."
                                );
                                return;
                            } else {
                                error!(
                                    ingress_id = %self.id(),
                                    tunnel_id = %&tunnel.id(),
                                    "Error while receiving UDP datagram from tunnel: {:?}",
                                    e
                                );
                            }
                        }
                    }
                }
                _ = process.token.cancelled() => {
                    debug!(
                        ingress_id = %self.id(),
                        tunnel_id = %&tunnel.id(),
                        "Closed tunnel => UDP socket"
                    );
                    tunnel.close().await;
                    return;
                }
            }
        }
    }

    async fn handle_tunnel_close(self, tunnel: Tunnel) {
        tunnel.closed().await;

        let mut tunnels = self.inner.tunnels.write();

        if let Some(i) = tunnels.iter().position(|t| t.id() == tunnel.id()) {
            tunnels.swap_remove(i);
        }
    }

    fn create_udp_socket(&self) -> Result<UdpSocket, Error> {
        let local_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

        #[cfg(unix)]
        socket.set_reuse_port(true)?;
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;

        let buffer_size = 16 * 1024 * 1024; // 16mb

        socket.set_recv_buffer_size(buffer_size)?;
        socket.set_send_buffer_size(buffer_size)?;

        socket.bind(&local_addr.into())?;

        let udp_socket = UdpSocket::from_std(socket.into())?;

        Ok(udp_socket)
    }
}
