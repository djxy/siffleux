use std::{sync::atomic::Ordering, time::Duration};

use futures_util::{SinkExt, StreamExt};
use quinn::{Connection, RecvStream, SendStream};
use tokio::time::sleep;
use tokio_util::codec::Framed;
use tracing::{debug, warn};

use crate::{
    Error, Server, TunnelId,
    code::{
        AUTH_FRAME_NOT_RECEIVED, AUTH_KEY_REJECTED, FIRST_FRAME_RECEIVED_NOT_AUTH,
        INGRESS_ID_REJECTED, SERVER_SIDE_ISSUE,
    },
    frames::v1::{CodecV1, FrameV1},
};

pub async fn handle_protocol_v1_auth(
    server: Server,
    connection: Connection,
    send: SendStream,
    recv: RecvStream,
) -> Result<(), Error> {
    let mut send_framed = Framed::new(send, CodecV1);
    let mut recv_framed = Framed::new(recv, CodecV1);

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
                warn!("protocol_v1_auth received no frame.");

                connection.close(AUTH_FRAME_NOT_RECEIVED.code, AUTH_FRAME_NOT_RECEIVED.reason);

                return Err(Error::AuthFrameNotReceived);
            }
        },
        _ = sleep(Duration::from_secs(5)) => {
            connection.close(AUTH_FRAME_NOT_RECEIVED.code, AUTH_FRAME_NOT_RECEIVED.reason);

            return Err(Error::AuthFrameNotReceived);
        }
    }

    Ok(())
}

pub async fn handle_protocol_v1_tcp_ingress_stream() {}
