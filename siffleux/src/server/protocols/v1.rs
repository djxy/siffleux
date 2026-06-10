use std::{sync::atomic::Ordering, time::Duration};

use futures_util::{SinkExt, StreamExt};
use quinn::{Connection, RecvStream, SendStream};
use tokio::{
    io::AsyncWriteExt,
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    sync::mpsc,
    time::sleep,
};
use tokio_util::codec::Framed;
use tracing::{debug, warn};

use crate::{
    Error, Server, Tunnel, TunnelId,
    code::{
        AUTH_FRAME_NOT_RECEIVED, AUTH_KEY_REJECTED, COMMAND_STREAM_CLOSED,
        FIRST_FRAME_RECEIVED_NOT_AUTH, INGRESS_ID_REJECTED, SERVER_SIDE_ISSUE,
        TCP_OR_QUIC_STREAM_FAILED,
    },
    frames::v1::{CodecV1, FrameV1},
};

pub async fn handle_protocol_v1_auth(
    server: Server,
    connection: Connection,
    mut send_framed: Framed<SendStream, CodecV1>,
    mut recv_framed: Framed<RecvStream, CodecV1>,
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

                                connection.close(INGRESS_ID_REJECTED.code, INGRESS_ID_REJECTED.reason);

                                return Err(Error::IngressIdRejected);
                            };

                            if !ingress.hashed_auth_key().verify(&auth_key) {
                                warn!(
                                    "Rejected auth_key from tunnel_name={}.",
                                    tunnel_name
                                );

                                connection.close(AUTH_KEY_REJECTED.code, AUTH_KEY_REJECTED.reason);

                                return Err(Error::AuthKeyRejected);
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
                            );

                            let _ = ingress.assign_tunnel(tunnel.clone());

                            handle_protocol_v1_command_stream(
                                connection,
                                tunnel,
                                send_framed,
                                recv_framed
                            );
                        }
                        _ => {
                            connection.close(FIRST_FRAME_RECEIVED_NOT_AUTH.code, FIRST_FRAME_RECEIVED_NOT_AUTH.reason);

                            return Err(Error::FirstFrameReceivedNotAuth);
                        }
                    }
                    Err(e) => {
                        connection.close(SERVER_SIDE_ISSUE.code, SERVER_SIDE_ISSUE.reason);

                        return Err(e);
                    }
                }
            } else {
                return Ok(());
            }
        },
        _ = sleep(Duration::from_secs(5)) => {
            connection.close(AUTH_FRAME_NOT_RECEIVED.code, AUTH_FRAME_NOT_RECEIVED.reason);

            return Err(Error::AuthFrameNotReceived);
        }
    }

    Ok(())
}

pub fn handle_protocol_v1_command_stream(
    connection: Connection,
    tunnel: Tunnel,
    mut send_framed: Framed<SendStream, CodecV1>,
    mut recv_framed: Framed<RecvStream, CodecV1>,
) {
    let (sender, mut receiver) = mpsc::channel::<FrameV1>(16);
    let connection_clone = connection.clone();

    tokio::spawn(async move {
        while let Some(frame) = receiver.recv().await {
            if let Err(e) = send_framed.send(frame).await {
                drop(receiver);

                connection_clone.close(SERVER_SIDE_ISSUE.code, SERVER_SIDE_ISSUE.reason);

                warn!("Error while sending ping {e}");

                return;
            }
        }
    });

    let sender_clone = sender.clone();
    let tunnel_id = tunnel.id().clone();

    tokio::spawn(async move {
        loop {
            if let Err(e) = sender_clone.send(FrameV1::Ping).await {
                return;
            }

            debug!("Sent ping to tunnel_id={}", tunnel_id);

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
                        debug!("Received pong from tunnel_id={}", tunnel_id);
                    }
                    _ => {}
                },
                Some(Err(e)) => {
                    connection.close(SERVER_SIDE_ISSUE.code, SERVER_SIDE_ISSUE.reason);
                    debug!("Command stream error for tunnel_id={}: {e}", tunnel.id());
                    return;
                }
                None => {
                    connection.close(COMMAND_STREAM_CLOSED.code, COMMAND_STREAM_CLOSED.reason);
                    debug!("Command stream closed for tunnel_id={}", tunnel.id());
                    return;
                }
            }
        }
    });
}

pub fn handle_protocol_v1_tcp_stream(
    mut send_stream: SendStream,
    mut recv_stream: RecvStream,
    mut tcp_read_stream: OwnedReadHalf,
    mut tcp_write_stream: OwnedWriteHalf,
) {
    tokio::spawn(async move {
        match tokio::io::copy(&mut tcp_read_stream, &mut send_stream).await {
            Ok(_) => {
                if let Err(e) = send_stream.finish() {
                    warn!("Failed to finish QUIC send stream correctly: {e}");
                }
            }
            Err(e) => {
                warn!("TCP read or QUIC send failed with error: {e}");

                let _ = send_stream.reset(TCP_OR_QUIC_STREAM_FAILED.code);
            }
        }
    });

    tokio::spawn(async move {
        match tokio::io::copy(&mut recv_stream, &mut tcp_write_stream).await {
            Ok(_) => {
                if let Err(e) = tcp_write_stream.shutdown().await {
                    warn!("Failed to finish TCP write correctly: {e}");
                }
            }
            Err(e) => {
                warn!("TCP read or QUIC write failed with error: {e}");
                let _ = recv_stream.stop(TCP_OR_QUIC_STREAM_FAILED.code);
            }
        }
    });
}
