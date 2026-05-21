use crate::common::error::Error;
use crate::common::tunnel::Tunnel;
use crate::common::types::IngressId;
use crate::server::ingress::ingress::Ingress;
use crate::server::server::Server;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct TcpIngress {
    inner: Arc<TcpIngressInner>,
}

struct TcpIngressInner {
    id: IngressId,
    socket_addr: SocketAddr,
    tunnels: RwLock<Vec<Tunnel>>,
    tcp_listener: Mutex<Option<Arc<TcpListener>>>,
    tunnel_rotation: AtomicUsize,
}

#[async_trait]
impl Ingress for TcpIngress {
    fn id(&self) -> &IngressId {
        &self.inner.id
    }

    fn assign_tunnel(&self, tunnel: Tunnel) -> Result<(), Error> {
        let tunnel_clone = tunnel.clone();
        let self_clone = self.clone();

        tokio::spawn(async move {
            tunnel_clone.closed().await;

            let mut tunnels = self_clone.inner.tunnels.write().unwrap();

            if let Some(i) = tunnels.iter().position(|t| t.id() == tunnel_clone.id()) {
                tunnels.swap_remove(i);
            }
        });

        self.inner.tunnels.write()?.push(tunnel);

        Ok(())
    }

    async fn start(&self, _server: &Server) -> Result<(), Error> {
        let tcp_listener = {
            let mut tcp_listener = self.inner.tcp_listener.lock().await;

            if tcp_listener.is_some() {
                return Err(Error::IngressAlreadyListening);
            }

            let tcp_listener_arc = Arc::new(TcpListener::bind(self.inner.socket_addr).await?);

            *tcp_listener = Some(tcp_listener_arc.clone());

            tcp_listener_arc
        };

        let self_clone = self.clone();

        tokio::spawn(async move {
            self_clone.handle_listener(tcp_listener).await.unwrap();
        });

        Ok(())
    }

    async fn stop(&self, _server: &Server) -> Result<(), Error> {
        if let Some(tcp_listener) = self.inner.tcp_listener.lock().await.take() {
            drop(tcp_listener);

            Ok(())
        } else {
            Err(Error::IngressNotListening)
        }
    }
}

impl TcpIngress {
    pub fn new(id: IngressId, socket_addr: SocketAddr) -> Self {
        Self {
            inner: Arc::new(TcpIngressInner {
                id,
                socket_addr,
                tunnels: RwLock::new(Vec::new()),
                tcp_listener: Mutex::new(None),
                tunnel_rotation: AtomicUsize::new(0),
            }),
        }
    }

    async fn handle_listener(&self, tcp_listener: Arc<TcpListener>) -> Result<(), Error> {
        let tcp_listener_cancellation_token = CancellationToken::new();

        while let Ok((tcp_stream, _)) = tcp_listener.accept().await {
            let self_clone = self.clone();
            let tcp_listener_cancellation_token_clone = tcp_listener_cancellation_token.clone();

            tokio::spawn(async move {
                self_clone
                    .handle_stream(tcp_stream, tcp_listener_cancellation_token_clone)
                    .await
                    .unwrap();
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
        let (mut tcp_read_stream, mut tcp_write_stream) = tcp_stream.into_split();

        if let Err(e) = tcp_read_stream.readable().await {
            drop(tcp_read_stream);
            drop(tcp_write_stream);
            return Err(e.into());
        }

        if let Err(e) = tcp_write_stream.writable().await {
            drop(tcp_read_stream);
            drop(tcp_write_stream);
            return Err(e.into());
        }

        let Ok(Some(tunnel)) = self.get_tunnel_to_connect() else {
            drop(tcp_read_stream);
            drop(tcp_write_stream);
            return Err(Error::IngressNoTunnelConnected);
        };

        let (mut tunnel_read_stream, mut tunnel_send_stream) = tunnel.create_stream().await?;
        let tcp_listener_cancellation_token_clone = tcp_listener_cancellation_token.clone();

        let tcp_to_tunnel_handle = tokio::spawn(async move {
            let mut buf = [0u8; 1024];

            // loop {
            //     select! {
            //         read_size_result = tcp_stream.read(&mut buf) => {
            //             match read_size_result {
            //                 Ok(0) => continue,
            //                 Ok(size) => {
            //                     tcp_stream.write_all(&buf[..size]).await?;
            //                 }
            //                 Err(_) => {
            //                     tcp_stream.shutdown().await?;
            //                     break;
            //                 },
            //             }
            //         }
            //         _ = self.inner.stop_token.cancelled() => {
            //             tcp_stream.shutdown().await?;
            //             break;
            //         }
            //     }
            // }
        });

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
