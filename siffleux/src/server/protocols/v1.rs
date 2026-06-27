use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use quinn::{Connection, RecvStream, SendStream};
use tokio::time::sleep;
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::{
    Error, Ingress, Server, Tunnel,
    code::{
        FRAME_NOT_RECEIVED_ON_TIME, REJECTED_AUTH_KEY, REJECTED_INGRESS_ID,
        UNEXPECTED_FRAME_RECEIVED, UNKNOWN_ERROR, UNKNOWN_ERROR_SERVER_REASON,
    },
    frames::v1::{CodecV1, FrameV1},
};

pub async fn handle_server_protocol_v1_auth(
    server: Server,
    connection: Connection,
    write_framed: &mut FramedWrite<SendStream, CodecV1>,
    read_framed: &mut FramedRead<RecvStream, CodecV1>,
) -> Result<(Box<dyn Ingress>, Tunnel), Error> {
    tokio::select! {
        frame_option = read_framed.next() => {
            if let Some(frame_res) = frame_option {
                match frame_res {
                    Ok(frame) => match frame {
                        FrameV1::Auth { auth_key, ingress_id} => {
                            let Some(ingress) = server.get_ingress_by_id(&ingress_id) else {
                                warn!("ingress_id={ingress_id} doesn't exist.");

                                connection.close(REJECTED_INGRESS_ID, b"rejected ingress id");

                                return Err(Error::RejectedIngressId);
                            };

                            if !ingress.hashed_auth_key().verify(&auth_key) {
                                warn!("Invalid auth key received.");

                                connection.close(REJECTED_AUTH_KEY, b"rejected auth key");

                                return Err(Error::RejectedAuthKey);
                            }

                            let tunnel_id = Uuid::now_v7();

                            debug!("Assigned tunnel_id={} to on ingress_id={}", tunnel_id, ingress_id);

                            write_framed
                                .send(FrameV1::Authenticated {
                                    tunnel_id,
                                    server_id: server.id().clone()
                                })
                                .await?;

                            return Ok((ingress, Tunnel::new(
                                tunnel_id,
                                server.id().clone(),
                                connection.clone(),
                                Some(server.byte_counter().clone())
                            )));
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

const PING_INTERVAL_SEC: u64 = 5;

pub async fn handle_server_protocol_v1_command_stream(
    tunnel: Tunnel,
    mut write_framed: FramedWrite<SendStream, CodecV1>,
    mut read_framed: FramedRead<RecvStream, CodecV1>,
) -> Result<(), Error> {
    let tunnel_id = tunnel.id();

    let ping_delay = tokio::time::sleep_until(
        tokio::time::Instant::now() + Duration::from_secs(PING_INTERVAL_SEC),
    );

    tokio::pin!(ping_delay);

    debug!(tunnel_id = %tunnel_id, "Start command handler");

    loop {
        tokio::select! {
            _ = &mut ping_delay => {
                write_framed.send(FrameV1::Ping).await?;

                debug!(tunnel_id = %tunnel_id, "Ping");

                ping_delay.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(PING_INTERVAL_SEC));
            }
            frame_opt = read_framed.next() => {
                match frame_opt {
                    Some(Ok(frame)) => match frame {
                        FrameV1::Ping => {
                            write_framed.send(FrameV1::Pong).await?;
                        }
                        FrameV1::Pong => {
                            debug!(tunnel_id = %tunnel_id, "Pong");
                        }
                        _ => {}
                    },
                    Some(Err(e)) => {
                        return match e {
                            Error::ClosedTunnel => Ok(()),
                            _ => Err(e)
                        }
                    }
                    None => {
                        return Ok(());
                    }
                }
            }
        }
    }
}
