use async_trait::async_trait;
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::common::AuthKey;
use crate::frames::v1::{extract_socket_addr_from_datagram, to_datagram};
use crate::{Error, Tunnel};
use crate::{Ingress, IngressId};

#[derive(Clone)]
pub struct UdpIngress {
    inner: Arc<UdpIngressInner>,
}

struct UdpIngressInner {
    id: IngressId,
    auth_key: AuthKey,
    /// Socket address the ingress will listen for UDP datagrams
    socket_addr: SocketAddr,
    udp_socket: tokio::sync::RwLock<Option<(Arc<UdpSocket>, CancellationToken)>>,
    tunnels: RwLock<Vec<Tunnel>>,
    tunnel_rotation: AtomicUsize,
}

#[async_trait]
impl Ingress for UdpIngress {
    fn id(&self) -> &IngressId {
        &self.inner.id
    }

    fn auth_key(&self) -> &AuthKey {
        &self.inner.auth_key
    }

    async fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
        let Some((udp_socket, cancellation_token)) = self
            .inner
            .udp_socket
            .read()
            .await
            .as_ref()
            .map(|(udp_socket, cancellation_token)| {
                (udp_socket.clone(), cancellation_token.clone())
            })
        else {
            return Err(Error::IngressNotListening);
        };

        let tunnel_clone = tunnel.clone();
        let self_clone = self.clone();
        let udp_socket = udp_socket.clone();
        let cancellation_token = cancellation_token.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    bytes_result = tunnel_clone.read_datagram() => {
                        match bytes_result {
                            Ok(mut bytes) => {
                                if let Some(origin_socket_addr) = extract_socket_addr_from_datagram(&mut bytes) {
                                    let _ = udp_socket.send_to(&bytes[..], origin_socket_addr).await;
                                };
                            }
                            Err(e) => {
                                error!(
                                    ingress_id = %self_clone.id(),
                                    tunnel_id = %&tunnel_clone.id(),
                                    "Error while receiving UDP datagram from tunnel: {:?}",
                                    e
                                );
                            }
                        }
                    }
                    _ = cancellation_token.cancelled() => {
                        debug!(
                            ingress_id = %self_clone.id(),
                            tunnel_id = %&tunnel_clone.id(),
                            "Closed tunnel => UDP socket"
                        );
                        tunnel_clone.close().await;
                        return;
                    }
                }
            }
        });

        let self_clone = self.clone();
        let tunnel_clone = tunnel.clone();

        tokio::spawn(async move {
            tunnel_clone.closed().await;

            let mut tunnels = self_clone.inner.tunnels.write();

            if let Some(i) = tunnels.iter().position(|t| t.id() == tunnel_clone.id()) {
                tunnels.swap_remove(i);
            }
        });

        info!(
            ingress_id = %self.id(),
            tunnel_id = %&tunnel.id(),
            "Assigned tunnel to UDP ingress."
        );

        self.inner.tunnels.write().push(tunnel);

        Ok(())
    }

    async fn start(&self) -> Result<(), Error> {
        let (udp_socket, cancellation_token) = {
            let mut udp_socket = self.inner.udp_socket.write().await;

            if udp_socket.is_some() {
                return Err(Error::IngressAlreadyListening);
            }

            info!(ingress_id = %self.id(), "Starting UDP ingress");

            let udp_socket_arc = Arc::new(UdpSocket::bind(self.inner.socket_addr).await?);
            let cancellation_token = CancellationToken::new();

            *udp_socket = Some((udp_socket_arc.clone(), cancellation_token.clone()));

            (udp_socket_arc, cancellation_token)
        };

        let self_clone = self.clone();
        let udp_socket_addr = udp_socket.local_addr().unwrap();

        tokio::spawn(async move {
            self_clone
                .handle_socket_to_tunnel(udp_socket, cancellation_token)
                .await;
        });

        info!(ingress_id = %self.id(), "Ready to receive UDP datagrams on {udp_socket_addr}.");

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        if let Some((udp_socket, cancellation_token)) = self.inner.udp_socket.write().await.take() {
            info!(ingress_id = %self.id(), "Stopping UDP ingress");
            cancellation_token.cancel();
            drop(udp_socket);

            Ok(())
        } else {
            Err(Error::IngressNotListening)
        }
    }
}

impl UdpIngress {
    pub fn new(id: IngressId, auth_key: AuthKey, socket_addr: SocketAddr) -> Self {
        Self {
            inner: Arc::new(UdpIngressInner {
                id,
                auth_key,
                socket_addr,
                tunnels: RwLock::new(Vec::new()),
                udp_socket: tokio::sync::RwLock::new(None),
                tunnel_rotation: AtomicUsize::new(0),
            }),
        }
    }

    pub async fn get_socket_addr(&self) -> Option<SocketAddr> {
        self.inner
            .udp_socket
            .read()
            .await
            .as_ref()
            .map(|udp_socket| udp_socket.0.local_addr().unwrap().clone())
    }

    async fn handle_socket_to_tunnel(
        &self,
        udp_socket: Arc<UdpSocket>,
        cancellation_token: CancellationToken,
    ) {
        let mut buffer = [0u8; 1500];

        loop {
            tokio::select! {
                result = udp_socket.recv_from(&mut buffer) => {
                    match result {
                        Ok((len, socket_addr)) => {
                            if let Some(tunnel) = self.get_tunnel_to_connect() {
                                if let Err(e) = tunnel.send_datagram(to_datagram(socket_addr, &buffer, len))
                                {
                                    error!(ingress_id = %self.id(), "Error while sending datagram to tunnel: {e}");
                                }
                            } else {
                                warn!(ingress_id = %self.id(), "No tunnel available to send datagram.");
                            }
                        }
                        Err(e) => {
                            error!(ingress_id = %self.id(), "Error while receiving datagram from socket: {e}");
                            break;
                        }
                    }
                }
                _ = cancellation_token.cancelled() => {
                    debug!(ingress_id = %self.id(), "Stopped socket => tunnel");
                    return;
                }
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
}
