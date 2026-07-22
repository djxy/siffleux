use std::{
    collections::HashMap,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    sync::Arc,
    time::Duration,
};

use bytes::{Buf, Bytes};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::{
    net::UdpSocket,
    sync::{
        RwLock,
        mpsc::{self, UnboundedSender},
    },
    task::JoinHandle,
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::{
    Egress, Error, IngressId, Tunnel,
    authentication::Authentication,
    client::egress::EgressId,
    frames::v1::{UDP_IPV4_ORIGIN, UDP_IPV6_ORIGIN, to_datagram},
};

#[derive(Clone)]
pub struct UdpEgress {
    inner: Arc<UdpEgressInner>,
}

struct UdpEgressInner {
    id: EgressId,
    ingress_id: IngressId,
    authentication: Box<dyn Authentication>,
    target_addr: SocketAddr,
    process: RwLock<Option<(CancellationToken, JoinHandle<()>)>>,
}

#[async_trait::async_trait]
impl Egress for UdpEgress {
    fn id(&self) -> &EgressId {
        &self.inner.id
    }

    fn ingress_id(&self) -> &IngressId {
        &self.inner.ingress_id
    }

    async fn start(&self) -> Result<(), Error> {
        let mut start_process = self.inner.process.write().await;

        if start_process.is_some() {
            return Err(Error::EgressAlreadyStarted);
        }

        info!(egress_id = %self.id(), "Starting");

        let mut tunnel = self.inner.authentication.connect(self).await?;

        info!(egress_id = %self.id(), tunnel_id = %tunnel.id(), "Tunnel connected");

        let cancellation_token = CancellationToken::new();
        let cancellation_token_clone = cancellation_token.clone();
        let self_clone = self.clone();

        let handle = tokio::spawn(async move {
            let (origin_expired_sender, mut origin_expired_receiver) =
                mpsc::unbounded_channel::<SocketAddr>();
            let mut udp_sockets: HashMap<SocketAddr, mpsc::Sender<Bytes>> =
                HashMap::with_capacity(64);

            loop {
                loop {
                    tokio::select! {
                        datagram_result = tunnel.read_datagram() => {
                            match datagram_result {
                                Ok(bytes) => {
                                    self_clone.handle_datagram(
                                        bytes,
                                        &tunnel,
                                        &mut udp_sockets,
                                        &origin_expired_sender,
                                        &cancellation_token_clone
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
                            warn!(egress_id = %self_clone.id(), "Tunnel with server closed.");
                            break;
                        }
                        _ = cancellation_token_clone.cancelled() => {
                            self_clone.stop_process(tunnel).await;
                            return;
                        }
                    }
                }

                let mut retry: u32 = 0;

                loop {
                    info!(egress_id = %self_clone.id(), "Reconnecting to server...");
                    match self_clone.inner.authentication.connect(&self_clone).await {
                        Ok(new_tunnel) => {
                            info!(egress_id = %self_clone.id(), "Reconnected to server.");
                            tunnel = new_tunnel;
                            break;
                        }
                        Err(_) => {
                            let duration =
                                Duration::from_millis((100 * 2_u64.pow(retry)).min(30_000_u64));

                            info!(egress_id = %self_clone.id(), "Failed to reconnect. Retry reconnecting in {:?}.", duration);

                            tokio::select! {
                                _ = sleep(duration) => {
                                    retry += 1;
                                }
                                _ = cancellation_token_clone.cancelled() => {
                                    self_clone.stop_process(tunnel).await;
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        });

        *start_process = Some((cancellation_token, handle));

        info!(egress_id = %self.id(), "Started");

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        match self.inner.process.write().await.take() {
            Some((cancellation_token, handle)) => {
                info!(egress_id = %self.id(), "Stopping");
                cancellation_token.cancel();
                handle.await?;
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
        Self {
            inner: Arc::new(UdpEgressInner {
                id,
                ingress_id,
                authentication,
                target_addr,
                process: RwLock::new(None),
            }),
        }
    }

    async fn stop_process(&self, tunnel: Tunnel) {
        tunnel.close().await;
    }

    fn handle_datagram(
        &self,
        mut bytes: Bytes,
        tunnel: &Tunnel,
        udp_sockets: &mut HashMap<SocketAddr, mpsc::Sender<Bytes>>,
        origin_expired_sender: &UnboundedSender<SocketAddr>,
        cancellation_token: &CancellationToken,
    ) {
        let Some(origin_socket_addr) = UdpEgress::extract_socket_addr(&mut bytes) else {
            return;
        };

        if let Some(bytes_sender) = udp_sockets.get(&origin_socket_addr) {
            let _ = bytes_sender.send(bytes);
        } else {
            let (bytes_sender, mut bytes_receiver) = mpsc::channel::<Bytes>(8);
            let tunnel = tunnel.clone();
            let origin_expired_sender = origin_expired_sender.clone();
            let cancellation_token = cancellation_token.clone();
            let self_clone = self.clone();

            let _ = bytes_sender.send(bytes);
            udp_sockets.insert(origin_socket_addr.clone(), bytes_sender);

            tokio::spawn(async move {
                let Ok(udp_socket) = self_clone.get_udp_socket().await else {
                    let _ = origin_expired_sender.send(origin_socket_addr);
                    return;
                };

                let mut buffer = [0u8; 1500];

                loop {
                    tokio::select! {
                        bytes_opt = bytes_receiver.recv() => {
                            if let Some(bytes) = bytes_opt {
                                let _ = udp_socket.send(&bytes[..]).await;
                            }
                        }
                        recv_result = udp_socket.recv(&mut buffer) => {
                            match recv_result {
                                Ok(len) => {
                                    let _ = tunnel.send_datagram(to_datagram(origin_socket_addr, &buffer, len));
                                }
                                Err(e) => {
                                    error!(egress_id = %self_clone.id(), "Error while receiving UDP packet: {:?}", e);
                                }
                            }
                        }
                        _ = tunnel.closed() => {
                            warn!(egress_id = %self_clone.id(), "Tunnel with server closed.");
                            let _ = origin_expired_sender.send(origin_socket_addr);
                            break;
                        }
                        _ = cancellation_token.cancelled() => {
                            self_clone.stop_process(tunnel).await;
                            break;
                        }
                        _ = sleep(Duration::from_secs(60)) => {
                            let _ = origin_expired_sender.send(origin_socket_addr);
                            break;
                        }
                    }
                }
            });
        }
    }

    fn extract_socket_addr(bytes: &mut Bytes) -> Option<SocketAddr> {
        match bytes.get_u8() {
            UDP_IPV4_ORIGIN => {
                let mut octets = [0u8; 4];

                bytes.copy_to_slice(&mut octets);

                Some(SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::from_octets(octets),
                    bytes.get_u16(),
                )))
            }
            UDP_IPV6_ORIGIN => {
                let mut octets = [0u8; 16];

                bytes.copy_to_slice(&mut octets);

                Some(SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::from_octets(octets),
                    bytes.get_u16(),
                    0,
                    0,
                )))
            }
            _ => None,
        }
    }

    async fn get_udp_socket(&self) -> Result<UdpSocket, Error> {
        let local_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

        #[cfg(unix)]
        socket.set_reuse_port(true)?;
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;
        socket.bind(&local_addr.into())?;

        let udp_socket = UdpSocket::from_std(socket.into())?;

        udp_socket.connect(self.inner.target_addr).await?;

        Ok(udp_socket)
    }
}
