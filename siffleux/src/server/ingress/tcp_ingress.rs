use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use crate::Ingress;
use crate::common::protocols::v1::handle_protocol_v1_tcp_stream;
use crate::common::{HashedAuthKey, IngressId};
use crate::{Error, Tunnel};

#[derive(Clone)]
pub struct TcpIngress {
    inner: Arc<TcpIngressInner>,
}

struct TcpIngressInner {
    id: IngressId,
    hashed_auth_key: HashedAuthKey,
    socket_addr: SocketAddr,
    tunnels: RwLock<Vec<Tunnel>>,
    tcp_listener: tokio::sync::Mutex<Option<Arc<TcpListener>>>,
    tcp_listener_socket_addr: Mutex<Option<SocketAddr>>,
    tunnel_rotation: AtomicUsize,
}

#[async_trait]
impl Ingress for TcpIngress {
    fn id(&self) -> &IngressId {
        &self.inner.id
    }

    fn hashed_auth_key(&self) -> &HashedAuthKey {
        &self.inner.hashed_auth_key
    }

    fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
        let tunnel_clone = tunnel.clone();
        let self_clone = self.clone();

        tokio::spawn(async move {
            tunnel_clone.connection().closed().await;

            let mut tunnels = self_clone.inner.tunnels.write().unwrap();

            if let Some(i) = tunnels.iter().position(|t| t.id() == tunnel_clone.id()) {
                tunnels.swap_remove(i);
            }
        });

        debug!(
            "Added tunnel_name={} to ingres_id={}",
            tunnel.name(),
            self.id()
        );

        self.inner.tunnels.write()?.push(tunnel);

        Ok(())
    }

    async fn start(&self) -> Result<(), Error> {
        let tcp_listener = {
            let mut tcp_listener = self.inner.tcp_listener.lock().await;

            if tcp_listener.is_some() {
                return Err(Error::IngressAlreadyListening);
            }

            debug!("Starting tcp ingress_id={}", self.id());

            let tcp_listener_arc = Arc::new(TcpListener::bind(self.inner.socket_addr).await?);

            debug!("Started tcp ingress_id={}", self.id());

            {
                let mut tcp_listener_socket_addr = self.inner.tcp_listener_socket_addr.lock()?;

                *tcp_listener_socket_addr = Some(tcp_listener_arc.local_addr()?.clone());
            }

            *tcp_listener = Some(tcp_listener_arc.clone());

            tcp_listener_arc
        };

        let self_clone = self.clone();

        tokio::spawn(async move {
            self_clone.handle_listener(tcp_listener).await.unwrap();
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        if let Some(tcp_listener) = self.inner.tcp_listener.lock().await.take() {
            drop(tcp_listener);

            Ok(())
        } else {
            Err(Error::IngressNotListening)
        }
    }
}

impl TcpIngress {
    pub fn new(id: IngressId, hashed_auth_key: HashedAuthKey, socket_addr: SocketAddr) -> Self {
        Self {
            inner: Arc::new(TcpIngressInner {
                id,
                hashed_auth_key,
                socket_addr,
                tunnels: RwLock::new(Vec::new()),
                tcp_listener: tokio::sync::Mutex::new(None),
                tcp_listener_socket_addr: Mutex::new(None),
                tunnel_rotation: AtomicUsize::new(0),
            }),
        }
    }

    pub fn socket_addr(&self) -> Result<Option<SocketAddr>, Error> {
        Ok(*self.inner.tcp_listener_socket_addr.lock()?)
    }

    async fn handle_listener(&self, tcp_listener: Arc<TcpListener>) -> Result<(), Error> {
        let tcp_listener_cancellation_token = CancellationToken::new();
        debug!("Tcp ingress_id={} waiting connections", self.id());

        while let Ok((tcp_stream, _)) = tcp_listener.accept().await {
            debug!("Received tcp connection on ingress_id={}", self.id());

            let self_clone = self.clone();
            let tcp_listener_cancellation_token_clone = tcp_listener_cancellation_token.clone();

            tokio::spawn(async move {
                if let Err(e) = self_clone
                    .handle_stream(tcp_stream, tcp_listener_cancellation_token_clone)
                    .await
                {
                    error!("Error while handling tcp connection in ingress. {}", e);
                }
            });
        }

        tcp_listener_cancellation_token.cancel();

        Ok(())
    }

    async fn handle_stream(
        &self,
        tcp_stream: TcpStream,
        tcp_listener_cancellation_token: CancellationToken,
    ) -> Result<(), Error> {
        let tcp_socket_addr = tcp_stream.local_addr()?;
        let (tcp_read_stream, tcp_write_stream): (OwnedReadHalf, OwnedWriteHalf) =
            tcp_stream.into_split();

        if let Err(e) = tcp_read_stream.readable().await {
            return Err(e.into());
        }

        if let Err(e) = tcp_write_stream.writable().await {
            return Err(e.into());
        }

        let Ok(Some(tunnel)) = self.get_tunnel_to_connect() else {
            return Err(Error::IngressNoTunnelConnected);
        };

        let (read_stream, write_stream, _) = tunnel.create_stream().await?;

        handle_protocol_v1_tcp_stream(
            read_stream,
            write_stream,
            tcp_socket_addr,
            tcp_read_stream,
            tcp_write_stream,
            tcp_listener_cancellation_token,
        );

        Ok(())
    }

    fn get_tunnel_to_connect(&self) -> Result<Option<Tunnel>, Error> {
        let tunnels = self.inner.tunnels.read()?;

        if tunnels.is_empty() {
            return Ok(None);
        }

        if tunnels.len() == 1 {
            return Ok(Some(tunnels[0].clone()));
        }

        Ok(Some(
            tunnels[self.inner.tunnel_rotation.fetch_add(1, Ordering::Relaxed) % tunnels.len()]
                .clone(),
        ))
    }
}
