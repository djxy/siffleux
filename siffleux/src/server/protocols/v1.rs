use std::{sync::atomic::Ordering, time::Duration};

use futures_util::{SinkExt, StreamExt};
use quinn::{Connection, RecvStream, SendStream};
use tokio::{sync::mpsc, time::sleep};
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::{debug, warn};

use crate::{
    Error, Server, Tunnel,
    code::{
        COMMAND_STREAM_CLOSED, FRAME_NOT_RECEIVED_ON_TIME, INVALID_VALUE,
        UNEXPECTED_FRAME_RECEIVED, UNKNOWN_ERROR, UNKNOWN_ERROR_SERVER_REASON,
    },
    common::TunnelId,
    frames::v1::{CodecV1, FrameV1},
};

pub async fn handle_server_protocol_v1_auth(
    server: Server,
    connection: Connection,
    mut send_framed: FramedWrite<SendStream, CodecV1>,
    mut recv_framed: FramedRead<RecvStream, CodecV1>,
) -> Result<(), Error> {
    tokio::select! {
        frame_option = recv_framed.next() => {
            if let Some(frame_res) = frame_option {
                match frame_res {
                    Ok(frame) => match frame {
                        FrameV1::Auth { auth_key, ingress_id, tunnel_name} => {
                            let Some(ingress) = server
                                .inner
                                .ingress_by_id
                                .read()
                                .unwrap()
                                .get(&ingress_id)
                                .cloned()
                            else {
                                warn!(
                                    "Rejected ingress_id from tunnel_name={}.",
                                    tunnel_name,
                                );

                                let msg = "Rejected ingress_id";

                                connection.close(INVALID_VALUE, msg.as_bytes());

                                return Err(Error::InvalidData(msg.to_string()));
                            };

                            if !ingress.hashed_auth_key().verify(&auth_key) {
                                warn!(
                                    "Rejected auth_key from tunnel_name={}.",
                                    tunnel_name
                                );

                                let msg = "Rejected auth_key";

                                connection.close(INVALID_VALUE, msg.as_bytes());

                                return Err(Error::InvalidData(msg.to_string()));
                            }

                            let tunnel_id = TunnelId::new(
                                server
                                    .inner
                                    .tunnel_id_counter
                                    .fetch_add(1, Ordering::SeqCst),
                            );

                            debug!(
                                "Assigned tunnel_id={} to tunnel_name={} on ingress_id={}",
                                tunnel_id, tunnel_name, ingress_id
                            );

                            send_framed
                                .send(FrameV1::Authenticated {
                                    tunnel_id: TunnelId::new(0),
                                })
                                .await?;

                            let tunnel = Tunnel::new(
                                tunnel_id,
                                tunnel_name,
                                ingress_id.clone(),
                                connection.clone(),
                                Some(server.byte_counter().clone())
                            );

                            let _ = ingress.assign_tunnel(tunnel.clone());

                            handle_server_protocol_v1_command_stream(
                                connection,
                                tunnel,
                                send_framed,
                                recv_framed
                            );

                            return Ok(());
                        }
                        _ => {
                            let msg = "Expected first frame to be auth.";

                            connection.close(UNEXPECTED_FRAME_RECEIVED, msg.as_bytes());

                            return Err(Error::UnexpectedFrameReceived(msg.to_string()));
                        }
                    }
                    Err(e) => {
                        connection.close(UNKNOWN_ERROR, UNKNOWN_ERROR_SERVER_REASON);

                        return Err(Error::Unknown(e.into()));
                    }
                }
            } else {
                return Err(Error::ConnectionClosed);
            }
        },
        _ = sleep(Duration::from_secs(5)) => {
            let msg = "Auth frame not received on time.";

            connection.close(FRAME_NOT_RECEIVED_ON_TIME, msg.as_bytes());

            return Err(Error::FrameNotReceivedOnTime(msg.to_string()));
        }
    }
}

pub fn handle_server_protocol_v1_command_stream(
    connection: Connection,
    tunnel: Tunnel,
    mut send_framed: FramedWrite<SendStream, CodecV1>,
    mut recv_framed: FramedRead<RecvStream, CodecV1>,
) {
    let (sender, mut receiver) = mpsc::channel::<FrameV1>(16);
    let connection_clone = connection.clone();
    let tunnel_id = tunnel.id();

    tokio::spawn(async move {
        while let Some(frame) = receiver.recv().await {
            if let Err(e) = send_framed.send(frame).await {
                drop(receiver);

                connection_clone.close(UNKNOWN_ERROR, UNKNOWN_ERROR_SERVER_REASON);

                warn!("Error while sending frame on command stream tunnel_id={tunnel_id}: {e}");

                return;
            }
        }
    });

    let sender_clone = sender.clone();

    tokio::spawn(async move {
        loop {
            if let Err(_) = sender_clone.send(FrameV1::Ping).await {
                return;
            }

            debug!("Ping tunnel_id={tunnel_id}");

            sleep(Duration::from_secs(5)).await;
        }
    });

    tokio::spawn(async move {
        loop {
            match recv_framed.next().await {
                Some(Ok(frame)) => match frame {
                    FrameV1::Ping => {
                        let _ = sender.send(FrameV1::Pong).await;
                    }
                    FrameV1::Pong => {
                        debug!("Pong tunnel_id={tunnel_id}");
                    }
                    _ => {}
                },
                Some(Err(e)) => {
                    connection.close(UNKNOWN_ERROR, UNKNOWN_ERROR_SERVER_REASON);
                    debug!("Command stream error on tunnel_id={tunnel_id}: {e}");
                    return;
                }
                None => {
                    connection.close(COMMAND_STREAM_CLOSED, b"Command stream closed.");
                    debug!("Command stream closed on tunnel_id={tunnel_id}");
                    return;
                }
            }
        }
    });
}
