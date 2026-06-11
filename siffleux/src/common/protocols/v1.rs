use tokio::{
    io::AsyncWriteExt,
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::{
    code::DATA_STREAM_ERROR,
    common::tunnel::{TunnelReadStream, TunnelWriteStream},
};

pub fn handle_protocol_v1_tcp_stream(
    mut tunnel_read_stream: TunnelReadStream,
    mut tunnel_write_stream: TunnelWriteStream,
    mut tcp_read_stream: OwnedReadHalf,
    mut tcp_write_stream: OwnedWriteHalf,
    cancellation_token: CancellationToken,
) {
    let stream_cancellation_token = cancellation_token.child_token();

    let stream_cancellation_token_clone = stream_cancellation_token.clone();

    tokio::spawn(async move {
        tokio::select! {
            copy_result = tokio::io::copy(&mut tcp_read_stream, &mut tunnel_write_stream) => {
                match copy_result {
                    Ok(_) => {
                        if let Err(e) = tunnel_write_stream.into_inner().finish() {
                            debug!("Failed to finish tunnel write stream: {e}");
                        }
                    }
                    Err(e) => {
                        warn!("TCP read stream or tunnel write stream failed with error: {e}");

                        let _ = tunnel_write_stream.into_inner().reset(DATA_STREAM_ERROR);

                        stream_cancellation_token_clone.cancel();
                    }
                }
            }
            _ = stream_cancellation_token_clone.cancelled() => {
                let _ = tunnel_write_stream.into_inner().reset(DATA_STREAM_ERROR);
            }
        }
    });

    tokio::spawn(async move {
        tokio::select! {
            copy_result = tokio::io::copy(&mut tunnel_read_stream, &mut tcp_write_stream) => {
                match copy_result {
                    Ok(_) => {
                        if let Err(e) = tcp_write_stream.shutdown().await {
                            debug!("Failed to finish tcp write stream: {e}");
                        }
                    }
                    Err(e) => {
                        warn!("TCP write stream or tunnel read stream failed with error: {e}");

                        let _ = tunnel_read_stream.into_inner().stop(DATA_STREAM_ERROR);

                        stream_cancellation_token.cancel();
                    }
                }
            }
            _ = stream_cancellation_token.cancelled() => {
                let _ = tcp_write_stream.shutdown().await;
                let _ = tunnel_read_stream.into_inner().stop(DATA_STREAM_ERROR);
            }
        }
    });
}
