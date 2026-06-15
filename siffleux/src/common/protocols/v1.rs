use std::net::SocketAddr;

use tokio::{
    io::AsyncWriteExt,
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::{
    IngressId,
    code::UNKNOWN_ERROR,
    common::tunnel::{TunnelReadStream, TunnelStream, TunnelWriteStream},
};

pub async fn handle_protocol_v1_tcp_stream(
    ingress_id: &IngressId,
    tunnel_stream: TunnelStream,
    mut tunnel_read_stream: TunnelReadStream,
    mut tunnel_write_stream: TunnelWriteStream,
    tcp_remote_addr: SocketAddr,
    mut tcp_read_stream: OwnedReadHalf,
    mut tcp_write_stream: OwnedWriteHalf,
    cancellation_token: CancellationToken,
) {
    let stream_cancellation_token = cancellation_token.child_token();
    let stream_cancellation_token_clone = stream_cancellation_token.clone();
    let tunnel_stream_id = tunnel_stream.id();

    info!(
        ingress_id = %ingress_id,
        remote = %tcp_remote_addr,
        stream_id = %tunnel_stream_id,
        "Started streaming TCP <-> Tunnel"
    );

    tokio::join!(
        async move {
            debug!(
                ingress_id = %ingress_id,
                remote = %tcp_remote_addr,
                stream_id = %tunnel_stream_id,
                "Streaming TCP read -> tunnel write"
            );

            tokio::select! {
                copy_result = tokio::io::copy(&mut tcp_read_stream, &mut tunnel_write_stream) => {
                    match copy_result {
                        Ok(_) => {
                            if let Err(e) = tunnel_write_stream.into_inner().finish() {
                                error!(
                                    ingress_id = %ingress_id,
                                    remote = %tcp_remote_addr,
                                    stream_id = %tunnel_stream_id,
                                    "TCP read closed and failed to finish tunnel write: {e}"
                                );
                            } else {
                                debug!(
                                    ingress_id = %ingress_id,
                                    remote = %tcp_remote_addr,
                                    stream_id = %tunnel_stream_id,
                                    "TCP read and tunnel write closed."
                                );
                            }
                        }
                        Err(e) => {
                            error!(
                                ingress_id = %ingress_id,
                                remote = %tcp_remote_addr,
                                stream_id = %tunnel_stream_id,
                                "TCP read or tunnel write failed with error: {e}"
                            );

                            let _ = tunnel_write_stream.into_inner().reset(UNKNOWN_ERROR);

                            stream_cancellation_token_clone.cancel();
                        }
                    }
                }
                _ = stream_cancellation_token_clone.cancelled() => {
                    let _ = tunnel_write_stream.into_inner().reset(UNKNOWN_ERROR);

                    debug!(
                        ingress_id = %ingress_id,
                        remote = %tcp_remote_addr,
                        stream_id = %tunnel_stream_id,
                        "TCP read -> tunnel write cancelled."
                    );
                }
            }

            debug!(
                ingress_id = %ingress_id,
                remote = %tcp_remote_addr,
                stream_id = %tunnel_stream_id,
                "TCP read -> tunnel write closed."
            );
        },
        async move {
            debug!(
                ingress_id = %ingress_id,
                remote = %tcp_remote_addr,
                stream_id = %tunnel_stream_id,
                "Streaming tunnel read -> TCP write"
            );

            tokio::select! {
                copy_result = tokio::io::copy(&mut tunnel_read_stream, &mut tcp_write_stream) => {
                    match copy_result {
                        Ok(_) => {
                            if let Err(e) = tcp_write_stream.shutdown().await {
                                error!(
                                    ingress_id = %ingress_id,
                                    remote = %tcp_remote_addr,
                                    stream_id = %tunnel_stream_id,
                                    "Tunnel read closed and failed to shutdown TCP write: {e}"
                                );
                            } else {
                                debug!(
                                    ingress_id = %ingress_id,
                                    remote = %tcp_remote_addr,
                                    stream_id = %tunnel_stream_id,
                                    "Tunnel read and TCP write closed."
                                );
                            }
                        }
                        Err(e) => {
                            error!(
                                ingress_id = %ingress_id,
                                remote = %tcp_remote_addr,
                                stream_id = %tunnel_stream_id,
                                "TCP write or tunnel read failed with error: {e}"
                            );

                            let _ = tcp_write_stream.shutdown().await;
                            let _ = tunnel_read_stream.into_inner().stop(UNKNOWN_ERROR);

                            stream_cancellation_token.cancel();
                        }
                    }
                }
                _ = stream_cancellation_token.cancelled() => {
                    let _ = tcp_write_stream.shutdown().await;
                    let _ = tunnel_read_stream.into_inner().stop(UNKNOWN_ERROR);

                    debug!(
                        ingress_id = %ingress_id,
                        remote = %tcp_remote_addr,
                        stream_id = %tunnel_stream_id,
                        "Tunnel read -> TCP write cancelled."
                    );
                }
            }

            debug!(
                ingress_id = %ingress_id,
                remote = %tcp_remote_addr,
                stream_id = %tunnel_stream_id,
                "Tunnel read -> TCP write closed.");
        }
    );

    info!(
        ingress_id = %ingress_id,
        remote = %tcp_remote_addr,
        stream_id = %tunnel_stream_id,
        "Finished streaming TCP <-> Tunnel"
    );
}
