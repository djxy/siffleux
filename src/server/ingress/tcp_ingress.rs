use crate::common::error::Error;
use crate::common::tunnel::Tunnel;
use crate::common::types::IngressId;
use crate::server::ingress::ingress::Ingress;
use crate::server::server::Server;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct TcpIngress {
    inner: Arc<TcpIngressInner>,
}

struct TcpIngressInner {
    id: IngressId,
    socket_addr: SocketAddr,
    tcp_listener: Mutex<Option<Arc<TcpListener>>>,
    stop_token: CancellationToken,
}

#[async_trait]
impl Ingress for TcpIngress {
    fn id(&self) -> &IngressId {
        &self.inner.id
    }

    fn assign_tunnel(&self, _tunnel: Tunnel) {}

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
            self.inner.stop_token.cancel();

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
                tcp_listener: Mutex::new(None),
                stop_token: CancellationToken::new(),
            }),
        }
    }

    async fn handle_listener(&self, tcp_listener: Arc<TcpListener>) -> Result<(), Error> {
        loop {
            while let Ok((mut tcp_stream, _)) = tcp_listener.accept().await {
                let self_clone = self.clone();

                tokio::spawn(async move {
                    self_clone.handle_socket(&mut tcp_stream).await.unwrap();
                });
            }
        }
    }

    async fn handle_socket(&self, tcp_stream: TcpStream) -> Result<(), Error> {
        let mut buf = [0u8; 1024];

        if let Err(e) = tcp_stream.readable().await {
            tcp_stream.shutdown().await?;
            return Err(e.into());
        }

        let (re, tcp_stream_write) = tcp_stream.into_split();
        
        let write_handle = tokio::spawn(async move {
            tcp_stream_write.write(buf.as_ref()).await;
        });

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

        Ok(())
    }
}
