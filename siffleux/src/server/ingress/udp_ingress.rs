use async_trait::async_trait;
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::common::AuthKey;
use crate::frames::v1::to_datagram;
use crate::{Error, Tunnel};
use crate::{Ingress, IngressId};

#[derive(Clone)]
pub struct UdpIngress {
    inner: Arc<UdpIngressInner>,
}

struct UdpIngressInner {
    id: IngressId,
    auth_key: AuthKey,
    /// Socket address the ingress will listen for UDP packets
    socket_addr: SocketAddr,
    udp_socket: Mutex<Option<Arc<UdpSocket>>>,
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

    fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
        let tunnel_clone = tunnel.clone();
        let self_clone = self.clone();

        info!(
            ingress_id = %self.id(),
            tunnel_id = %&tunnel.id(),
            "Assigned tunnel to UDP ingress."
        );

        tokio::spawn(async move {
            tunnel_clone.closed().await;

            let mut tunnels = self_clone.inner.tunnels.write();

            if let Some(i) = tunnels.iter().position(|t| t.id() == tunnel_clone.id()) {
                tunnels.swap_remove(i);
            }
        });

        self.inner.tunnels.write().push(tunnel);

        Ok(())
    }

    async fn start(&self) -> Result<(), Error> {
        let udp_socket = {
            let mut udp_socket = self.inner.udp_socket.lock().await;

            if udp_socket.is_some() {
                return Err(Error::IngressAlreadyListening);
            }

            info!(ingress_id = %self.id(), "Starting UDP ingress...");

            let udp_socket_arc = Arc::new(UdpSocket::bind(self.inner.socket_addr).await?);

            *udp_socket = Some(udp_socket_arc.clone());

            udp_socket_arc
        };

        let self_clone = self.clone();

        tokio::spawn(async move {
            self_clone.handle_socket(udp_socket).await.unwrap();
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        if let Some(udp_socket) = self.inner.udp_socket.lock().await.take() {
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
                udp_socket: Mutex::new(None),
                tunnel_rotation: AtomicUsize::new(0),
            }),
        }
    }

    async fn handle_socket(&self, udp_socket: Arc<UdpSocket>) -> Result<(), Error> {
        info!(ingress_id = %self.id(), "Ready to receive UDP packets on {}.", self.inner.socket_addr);

        let mut buffer = [0u8; 1500];

        loop {
            match udp_socket.recv_from(&mut buffer).await {
                Ok((len, socket_addr)) => {
                    if let Some(tunnel) = self.get_tunnel_to_connect() {
                        if let Err(e) = tunnel.send_datagram(to_datagram(socket_addr, &buffer, len))
                        {
                            error!("Error while sending datagram to tunnel: {e}");
                        }
                    } else {
                        warn!("No tunnel available to send datagram.");
                    }
                }
                Err(e) => {
                    error!("Error while receiving datagram from socket: {e}");
                    break;
                }
            }
        }

        Ok(())
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
