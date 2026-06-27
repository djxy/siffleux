use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use quinn::{Connection, RecvStream, SendStream};
use tokio::time::sleep;
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::debug;

use crate::{
    AuthKey, ByteCounter, Error, IngressId, Tunnel,
    code::{
        COMMAND_STREAM_CLOSED, FRAME_NOT_RECEIVED_ON_TIME, UNEXPECTED_FRAME_RECEIVED,
        UNKNOWN_ERROR, UNKNOWN_ERROR_CLIENT_REASON,
    },
    frames::v1::{CodecV1, FrameV1},
};

pub async fn handle_client_protocol_v1_auth(
    connection: Connection,
    auth_key: &AuthKey,
    ingress_id: &IngressId,
    client_byte_counter: &ByteCounter,
) -> Result<Tunnel, Error> {
    let (send_stream, recv_stream) = connection.open_bi().await?;
    let mut tunnel_write_framed: FramedWrite<SendStream, CodecV1> =
        FramedWrite::new(send_stream, CodecV1);
    let mut tunnel_read_framed: FramedRead<RecvStream, CodecV1> =
        FramedRead::new(recv_stream, CodecV1);

    debug!("Sending auth frame to server");

    tunnel_write_framed
        .send(FrameV1::Auth {
            auth_key: auth_key.clone(),
            ingress_id: ingress_id.clone(),
        })
        .await?;

    debug!("Waiting server response");

    tokio::select! {
        frame_option = tunnel_read_framed.next() => {
            if let Some(frame_res) = frame_option {
                match frame_res {
                    Ok(frame) => match frame {
                        FrameV1::Authenticated { server_id, tunnel_id } => {
                            debug!("Received authenticated frame. Assigned tunnel_id={tunnel_id}");

                            let tunnel = Tunnel::new(
                                tunnel_id,
                                server_id,
                                connection.clone(),
                                Some(client_byte_counter.clone())
                            );

                            handle_client_protocol_v1_command_stream(
                                connection,
                                tunnel.clone(),
                                tunnel_write_framed,
                                tunnel_read_framed
                            );

                            return Ok(tunnel);
                        }
                        _ => {
                            let msg = "First frame received wasn't authenticated.";

                            connection.close(UNEXPECTED_FRAME_RECEIVED, msg.as_bytes());

                            return Err(Error::UnexpectedFrameReceived(msg.to_string()));
                        }
                    }
                    Err(e) => {
                        connection.close(UNKNOWN_ERROR, UNKNOWN_ERROR_CLIENT_REASON);

                        return Err(e);
                    }
                }
            } else {
                return Err(Error::ConnectionClosed);
            }
        },
        _ = sleep(Duration::from_secs(5)) => {
            let msg = "Authenticated frame not received on time.";

            connection.close(FRAME_NOT_RECEIVED_ON_TIME, msg.as_bytes());

            return Err(Error::FrameNotReceivedOnTime(msg.to_string()));
        }
    }
}

const PING_INTERVAL_SEC: u64 = 5;

pub fn handle_client_protocol_v1_command_stream(
    connection: Connection,
    tunnel: Tunnel,
    mut send_framed: FramedWrite<SendStream, CodecV1>,
    mut recv_framed: FramedRead<RecvStream, CodecV1>,
) {
    tokio::spawn(async move {
        let connection_clone = connection.clone();
        let tunnel_id = tunnel.server_id();

        let ping_delay = tokio::time::sleep_until(
            tokio::time::Instant::now() + Duration::from_secs(PING_INTERVAL_SEC),
        );

        tokio::pin!(ping_delay);

        debug!("Start command handler tunnel_id={tunnel_id}");

        loop {
            tokio::select! {
                _ = &mut ping_delay => {
                    if let Err(e) = send_framed.send(FrameV1::Ping).await {
                        connection_clone.close(UNKNOWN_ERROR, UNKNOWN_ERROR_CLIENT_REASON);
                        debug!("Error while sending ping to tunnel_id={tunnel_id}: {e}");
                        return;
                    }

                    debug!("Ping tunnel_id={tunnel_id}");

                    ping_delay.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(PING_INTERVAL_SEC));
                }
                frame_opt = recv_framed.next() => {
                    match frame_opt {
                        Some(Ok(frame)) => match frame {
                            FrameV1::Ping => {
                                let _ = send_framed.send(FrameV1::Pong).await;
                            }
                            FrameV1::Pong => {
                                debug!("Pong tunnel_id={tunnel_id}");
                            }
                            _ => {}
                        },
                        Some(Err(e)) => {
                            connection_clone.close(UNKNOWN_ERROR, UNKNOWN_ERROR_CLIENT_REASON);
                            debug!("Command stream error on tunnel_id={tunnel_id}: {e}");
                            return;
                        }
                        None => {
                            connection_clone.close(COMMAND_STREAM_CLOSED, b"Command stream closed.");
                            debug!("Command stream closed on tunnel_id={tunnel_id}");
                            return;
                        }
                    }
                }
            }
        }
    });
}
