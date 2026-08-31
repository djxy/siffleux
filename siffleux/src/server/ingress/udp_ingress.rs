use parking_lot::RwLock;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, watch};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::common::AuthKey;
use crate::frames::v1::{extract_origin_socket_addr_from_datagram, to_datagram};
use crate::{Error, State, Tunnel};
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
    process_lock: Mutex<()>,
    process: RwLock<Option<Arc<UdpIngressProcess>>>,
    tunnels: RwLock<Vec<Tunnel>>,
    tunnel_rotation: AtomicUsize,
    state_sender: watch::Sender<State>,
    state_receiver: watch::Receiver<State>,
}

#[async_trait::async_trait]
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

    async fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
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

    async fn start(&self) -> Result<(), Error> {
        let _lock = self.inner.process_lock.lock().await;

        if self.inner.process.read().is_some() {
            return Err(Error::IngressAlreadyStarted);
        }

        let process = Arc::new(UdpIngressProcess {
            udp_socket: self.create_udp_socket()?,
            token: CancellationToken::new(),
        });

        info!(ingress_id = %self.id(), "Starting UDP ingress");

        {
            let mut process_guard = self.inner.process.write();
            *process_guard = Some(process.clone());
        }

        tokio::spawn(self.clone().start_socket_to_tunnel(process));

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

impl UdpIngress {
    pub fn new(id: IngressId, auth_key: AuthKey, listen_addr: SocketAddr) -> Self {
        let (state_sender, state_receiver) = watch::channel(State::Stopped);

        Self {
            inner: Arc::new(UdpIngressInner {
                id,
                auth_key,
                listen_addr,
                process_lock: Mutex::new(()),
                process: RwLock::new(None),
                tunnels: RwLock::new(Vec::new()),
                tunnel_rotation: AtomicUsize::new(0),
                state_sender,
                state_receiver,
            }),
        }
    }

    async fn start_socket_to_tunnel(self, process: Arc<UdpIngressProcess>) {
        info!(ingress_id = %self.id(), "Ready to receive UDP datagrams on {}.", process.udp_socket.local_addr().unwrap());

        let _ = self.inner.state_sender.send(State::Started);

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
                            match e.kind() {
                                std::io::ErrorKind::ConnectionReset |
                                std::io::ErrorKind::ConnectionRefused |
                                std::io::ErrorKind::Interrupted => {
                                    debug!(ingress_id = %self.id(), "Debug error: {e}");
                                    continue;
                                }
                                _ => {
                                    error!(ingress_id = %self.id(), "Error receiving datagram: {e}");
                                    break;
                                }
                            }
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
        let socket = Socket::new(
            Domain::for_address(self.inner.listen_addr),
            Type::DGRAM,
            Some(Protocol::UDP),
        )?;

        #[cfg(unix)]
        socket.set_reuse_port(true)?;
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;

        let buffer_size = 16 * 1024 * 1024; // 16mb

        socket.set_recv_buffer_size(buffer_size)?;
        socket.set_send_buffer_size(buffer_size)?;

        socket.bind(&self.inner.listen_addr.into())?;

        let udp_socket = UdpSocket::from_std(socket.into())?;

        Ok(udp_socket)
    }
}
