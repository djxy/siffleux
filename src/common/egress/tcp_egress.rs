use std::{net::SocketAddr, sync::Arc};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    select,
};
use tracing::error;

use crate::{Error, ReadChannel, Tunnel, WriteChannel, egress::Egress};

#[derive(Clone)]
pub struct TcpEgress {
    inner: Arc<TcpEgressInner>,
}

struct TcpEgressInner {
    tunnel: Tunnel,
    target_addr: SocketAddr,
}

#[async_trait::async_trait]
impl Egress for TcpEgress {
    async fn start(&self) -> Result<(), Error> {
        let self_clone = self.clone();

        tokio::spawn(async move {
            self_clone.listen_streams().await;
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        Ok(())
    }
}

impl TcpEgress {
    pub fn new(tunnel: Tunnel, target_addr: SocketAddr) -> Self {
        Self {
            inner: Arc::new(TcpEgressInner {
                tunnel,
                target_addr,
            }),
        }
    }

    async fn listen_streams(&self) {
        while let Ok((read_channel, write_channel)) = self.inner.tunnel.accept_stream().await {
            let self_clone = self.clone();

            tokio::spawn(async move {
                if let Err(e) = self_clone.handle_stream(read_channel, write_channel).await {
                    error!("Error while handling tcp stream on egress: {:?}", e);
                }
            });
        }

        self.close_listener();
    }

    async fn handle_stream(
        &self,
        read_channel: ReadChannel,
        write_channel: WriteChannel,
    ) -> Result<(), Error> {
        let (tcp_read, tcp_write) = TcpStream::connect(self.inner.target_addr)
            .await?
            .into_split();

        let self_tunnel_to_tcp = self.clone();

        tokio::spawn(async move {
            self_tunnel_to_tcp
                .handle_tunnel_to_tcp(read_channel, tcp_write)
                .await
                .unwrap();
        });

        let self_tcp_to_tunnel = self.clone();

        tokio::spawn(async move {
            self_tcp_to_tunnel
                .handle_tcp_to_tunnel(write_channel, tcp_read)
                .await
                .unwrap();
        });

        Ok(())
    }

    async fn handle_tunnel_to_tcp(
        &self,
        mut read_channel: ReadChannel,
        mut tcp_write: OwnedWriteHalf,
    ) -> Result<(), Error> {
        let mut buffer = [0u8; 1024];

        while let Ok(Some(size)) = read_channel.read(&mut buffer).await {
            if let Err(_) = tcp_write.write(&mut buffer[..size]).await {
                break;
            }
        }

        read_channel.close()?;
        drop(tcp_write);

        Ok(())
    }

    async fn handle_tcp_to_tunnel(
        &self,
        mut write_channel: WriteChannel,
        mut tcp_read: OwnedReadHalf,
    ) -> Result<(), Error> {
        let mut buffer = [0u8; 1024];
        let stream = write_channel.stream().clone();

        loop {
            select! {
                read_size_result = tcp_read.read(&mut buffer) {
                    match read_size_result {
                        Ok(0) => break,
                        Ok(size) => {
                            match write_channel.write(&mut buf[..size]).await {
                                Ok(_) => continue,
                                Err(_) => break,
                            }
                        },
                        Err(_) => break,
                    }
                }
                _ = stream.closed()
            }
        }

        while let Ok(size) = tcp_read.read(&mut buffer).await {
            if let Err(_) = write_channel.write(&mut buffer[..size]).await {
                break;
            };
        }

        write_channel.close()?;
        drop(tcp_read);

        Ok(())
    }

    fn close_listener(&self) {}
}
