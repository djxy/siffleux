use std::{net::SocketAddr, sync::Arc};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    select,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{Error, Tunnel, client::egress::Egress};

#[derive(Clone)]
pub struct TcpEgress {
    inner: Arc<TcpEgressInner>,
}

struct TcpEgressInner {
    tunnel: Tunnel,
    target_addr: SocketAddr,
    cancellation_token: CancellationToken,
}

#[async_trait::async_trait]
impl Egress for TcpEgress {
    async fn start(&self) -> Result<(), Error> {
        let cancellation_token = {
            let mut guard = self.inner.cancellation_token.lock()?;

            if guard.is_some() {
                return Err(Error::EgressAlreadyListening);
            }

            let ct = CancellationToken::new();

            *guard = Some(ct.clone());

            ct
        };

        info!("Started tcp egress targeting={}", self.inner.target_addr);

        let self_clone = self.clone();

        tokio::spawn(async move {
            self_clone.listen_streams(cancellation_token).await;
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        match self.inner.cancellation_token.lock()?.take() {
            Some(cancellation_token) => {
                cancellation_token.cancel();

                Ok(())
            }
            None => Err(Error::EgressNotListening),
        }
    }
}

impl TcpEgress {
    fn new(tunnel: Tunnel, target_addr: SocketAddr) -> Self {
        Self {
            inner: Arc::new(TcpEgressInner {
                tunnel,
                target_addr,
                cancellation_token: CancellationToken::new(),
            }),
        }
    }

    async fn listen_streams(&self, cancellation_token: CancellationToken) {
        while let Ok((read_channel, write_channel)) = self.inner.tunnel.accept_stream().await {
            info!(
                "Received stream from tunnel_id={} ingress_id={} on tcp_egress={}",
                self.inner.tunnel.id(),
                self.inner.tunnel.ingress_id(),
                self.inner.target_addr
            );

            let self_clone = self.clone();
            let ct = cancellation_token.clone();

            tokio::spawn(async move {
                if let Err(e) = self_clone
                    .handle_stream(read_channel, write_channel, ct)
                    .await
                {
                    error!("Error while handling tcp stream on egress: {}", e);
                }
            });
        }

        if let Err(e) = self.stop().await {
            error!("Error while stopping tcp egress: {}", e)
        }
    }

    async fn handle_stream(
        &self,
        read_channel: ReadChannel,
        write_channel: WriteChannel,
        cancellation_token: CancellationToken,
    ) -> Result<(), Error> {
        let (tcp_read, tcp_write) = TcpStream::connect(self.inner.target_addr)
            .await?
            .into_split();

        let self_tunnel_to_tcp = self.clone();
        let ct_tunnel_to_tcp = cancellation_token.clone();

        tokio::spawn(async move {
            self_tunnel_to_tcp
                .handle_tunnel_to_tcp(read_channel, tcp_write, ct_tunnel_to_tcp)
                .await
                .unwrap();
        });

        let self_tcp_to_tunnel = self.clone();
        let ct_tcp_to_tunnel = cancellation_token.clone();

        tokio::spawn(async move {
            self_tcp_to_tunnel
                .handle_tcp_to_tunnel(write_channel, tcp_read, ct_tcp_to_tunnel)
                .await
                .unwrap();
        });

        Ok(())
    }

    async fn handle_tunnel_to_tcp(
        &self,
        mut read_channel: ReadChannel,
        mut tcp_write: OwnedWriteHalf,
        cancellation_token: CancellationToken,
    ) -> Result<(), Error> {
        let mut buffer = [0u8; 1024];
        let stream = read_channel.stream().clone();

        loop {
            select! {
                read_size_result = read_channel.read(&mut buffer) => {
                    match read_size_result {
                        Ok(None) => break,
                        Ok(Some(size)) => {
                            match tcp_write.write(&mut buffer[..size]).await {
                                Ok(_) => continue,
                                Err(_) => break,
                            }
                        },
                        Err(_) => break,
                    }
                },
                _ = stream.closed() => break,
                _ = cancellation_token.cancelled() => break,
            }
        }

        info!(
            "Tunnel-to-tcp connection closed for tunnel_id={} ingress_id={} on tcp_egress={}",
            self.inner.tunnel.id(),
            self.inner.tunnel.ingress_id(),
            self.inner.target_addr
        );

        read_channel.close()?;

        Ok(())
    }

    async fn handle_tcp_to_tunnel(
        &self,
        mut write_channel: WriteChannel,
        mut tcp_read: OwnedReadHalf,
        cancellation_token: CancellationToken,
    ) -> Result<(), Error> {
        let mut buffer = [0u8; 1024];
        let stream = write_channel.stream().clone();

        loop {
            select! {
                read_size_result = tcp_read.read(&mut buffer) => {
                    match read_size_result {
                        Ok(0) => break,
                        Ok(size) => {
                            match write_channel.write(&mut buffer[..size]).await {
                                Ok(_) => continue,
                                Err(_) => break,
                            }
                        },
                        Err(_) => break,
                    }
                },
                _ = stream.closed() => break,
                _ = cancellation_token.cancelled() => break,
            }
        }

        info!(
            "Tcp-to-tunnel connection closed for tunnel_id={} ingress_id={} on tcp_egress={}",
            self.inner.tunnel.id(),
            self.inner.tunnel.ingress_id(),
            self.inner.target_addr
        );

        write_channel.close()?;

        Ok(())
    }
}
