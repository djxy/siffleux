use std::net::SocketAddr;

use tokio::{
    io::AsyncWriteExt,
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::{
    code::UNKNOWN_ERROR,
    common::tunnel::{TunnelReadStream, TunnelWriteStream},
};

pub fn handle_protocol_v1_tcp_stream(
    mut tunnel_read_stream: TunnelReadStream,
    mut tunnel_write_stream: TunnelWriteStream,
    tcp_socket_addr: SocketAddr,
    mut tcp_read_stream: OwnedReadHalf,
    mut tcp_write_stream: OwnedWriteHalf,
    cancellation_token: CancellationToken,
) -> JoinHandle<()> {
    let stream_cancellation_token = cancellation_token.child_token();
    let stream_cancellation_token_clone = stream_cancellation_token.clone();

    tokio::spawn(async move {
        tokio::join!(
            async move {
                debug!("Streaming TCP read {tcp_socket_addr} -> tunnel write stream");

                tokio::select! {
                    copy_result = tokio::io::copy(&mut tcp_read_stream, &mut tunnel_write_stream) => {
                        match copy_result {
                            Ok(_) => {
                                if let Err(e) = tunnel_write_stream.into_inner().finish() {
                                    debug!("TCP read {tcp_socket_addr} closed and failed to finish tunnel write stream: {e}");
                                } else {
                                    debug!("TCP read {tcp_socket_addr} and tunnel write stream closed.");
                                }
                            }
                            Err(e) => {
                                warn!("TCP read {tcp_socket_addr} or tunnel write stream failed with error: {e}");

                                let _ = tunnel_write_stream.into_inner().reset(UNKNOWN_ERROR);

                                stream_cancellation_token_clone.cancel();
                            }
                        }
                    }
                    _ = stream_cancellation_token_clone.cancelled() => {
                        let _ = tunnel_write_stream.into_inner().reset(UNKNOWN_ERROR);

                        debug!("TCP read {tcp_socket_addr} -> tunnel write stream cancelled.");
                    }
                }

                debug!("TCP read {tcp_socket_addr} -> tunnel write stream closed.");
            },
            async move {
                debug!("Streaming tunnel read stream -> TCP write {tcp_socket_addr}");

                tokio::select! {
                    copy_result = tokio::io::copy(&mut tunnel_read_stream, &mut tcp_write_stream) => {
                        match copy_result {
                            Ok(_) => {
                                if let Err(e) = tcp_write_stream.shutdown().await {
                                    debug!("Tunnel read stream closed and failed to shutdown TCP write {tcp_socket_addr}: {e}");
                                } else {
                                    debug!("Tunnel read stream and TCP write {tcp_socket_addr} closed.");
                                }
                            }
                            Err(e) => {
                                debug!("TCP write {tcp_socket_addr} or tunnel read stream failed with error: {e}");

                                let _ = tcp_write_stream.shutdown().await;
                                let _ = tunnel_read_stream.into_inner().stop(UNKNOWN_ERROR);

                                stream_cancellation_token.cancel();
                            }
                        }
                    }
                    _ = stream_cancellation_token.cancelled() => {
                        let _ = tcp_write_stream.shutdown().await;
                        let _ = tunnel_read_stream.into_inner().stop(UNKNOWN_ERROR);

                        debug!("Tunnel read stream -> tcp write {tcp_socket_addr} cancelled.");
                    }
                }

                debug!("Tunnel read stream -> tcp write {tcp_socket_addr} closed.");
            }
        );

        debug!("TCP {tcp_socket_addr} and tunnel stream closed");
    })
}
