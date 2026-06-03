use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::common::tunnel::{ReadChannel, WriteChannel};
use crate::ingress::Ingress;
use crate::{Error, IngressId, Tunnel};

#[derive(Clone)]
pub struct TcpIngress {
    inner: Arc<TcpIngressInner>,
}

struct TcpIngressInner {
    id: IngressId,
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

    async fn start(&self) -> Result<(), Error> {
        let tcp_listener = {
            let mut tcp_listener = self.inner.tcp_listener.lock().await;

            if tcp_listener.is_some() {
                return Err(Error::IngressAlreadyListening);
            }

            info!("Starting tcp ingress_id={}", self.id());

            let tcp_listener_arc = Arc::new(TcpListener::bind(self.inner.socket_addr).await?);

            info!("Started tcp ingress_id={}", self.id());

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
    pub fn new(id: IngressId, socket_addr: SocketAddr) -> Self {
        Self {
            inner: Arc::new(TcpIngressInner {
                id,
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
        info!("Tcp ingress_id={} waiting connections", self.id());

        while let Ok((tcp_stream, _)) = tcp_listener.accept().await {
            info!("Received tcp connection on ingress_id={}", self.id());

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

        let (read_channel, write_channel) = tunnel.create_stream().await?;
        let tcp_stream_cancellation_token = CancellationToken::new();

        self.handle_tcp_to_tunnel(
            tcp_listener_cancellation_token.clone(),
            tcp_stream_cancellation_token.clone(),
            tcp_read_stream,
            write_channel,
        );

        self.handle_tunnel_to_tcp(
            tcp_listener_cancellation_token,
            tcp_stream_cancellation_token,
            read_channel,
            tcp_write_stream,
        );

        Ok(())
    }

    fn handle_tcp_to_tunnel(
        &self,
        tcp_listener_cancellation_token: CancellationToken,
        tcp_stream_cancellation_token: CancellationToken,
        mut tcp_read_stream: OwnedReadHalf,
        mut write_channel: WriteChannel,
    ) {
        let self_clone = self.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let stream = write_channel.stream().clone();

            loop {
                select! {
                    read_size_result = tcp_read_stream.read(&mut buf) => {
                        match read_size_result {
                            Ok(0) => break,
                            Ok(size) => {
                                match write_channel.write(&mut buf[..size]).await {
                                    Ok(_) => continue,
                                    Err(_) => break,
                                }
                            }
                            Err(_) => break,
                        }
                    },
                    _ = stream.closed() => break,
                    _ = tcp_stream_cancellation_token.cancelled() => break,
                    _ = tcp_listener_cancellation_token.cancelled() => break,
                }
            }

            let _ = write_channel.close();
            tcp_stream_cancellation_token.cancel();

            info!(
                "Tcp ingress_id={} tcp-to-tunnel connection closed",
                self_clone.id()
            );
        });
    }

    fn handle_tunnel_to_tcp(
        &self,
        tcp_listener_cancellation_token: CancellationToken,
        tcp_stream_cancellation_token: CancellationToken,
        mut read_channel: ReadChannel,
        mut tcp_write_stream: OwnedWriteHalf,
    ) {
        let self_clone = self.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let stream = read_channel.stream().clone();

            loop {
                select! {
                    read_size_result = read_channel.read(&mut buf) => {
                        match read_size_result {
                            Ok(Some(0)) => continue,
                            Ok(Some(size)) => {
                                tcp_write_stream.write(&mut buf[..size]).await.unwrap();
                            }
                            Ok(None) => break,
                            Err(_) => break,
                        }
                    }
                    _ = stream.closed() => break,
                    _ = tcp_stream_cancellation_token.cancelled() => break,
                    _ = tcp_listener_cancellation_token.cancelled() => break,
                }
            }

            let _ = read_channel.close();
            tcp_stream_cancellation_token.cancel();

            info!(
                "Tcp ingress_id={} tunnel-to-tcp connection closed",
                self_clone.id()
            );
        });
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
