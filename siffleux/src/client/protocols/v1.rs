use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use quinn::{Connection, RecvStream, SendStream};
use tokio::time::sleep;
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::debug;

use crate::{
    Error, Tunnel,
    code::{
        FRAME_NOT_RECEIVED_ON_TIME, UNEXPECTED_FRAME_RECEIVED, UNKNOWN_ERROR,
        UNKNOWN_ERROR_CLIENT_REASON,
    },
    common::{AuthKey, ByteCounter, IngressId, TunnelName},
    frames::v1::{CodecV1, FrameV1},
};

pub async fn handle_protocol_v1_auth(
    connection: Connection,
    auth_key: AuthKey,
    ingress_id: IngressId,
    tunnel_name: TunnelName,
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
            tunnel_name: tunnel_name.clone(),
        })
        .await?;

    debug!("Waiting server response");

    tokio::select! {
        frame_option = tunnel_read_framed.next() => {
            if let Some(frame_res) = frame_option {
                match frame_res {
                    Ok(frame) => match frame {
                        FrameV1::Authenticated { tunnel_id } => {
                            debug!("Received authenticated frame for tunnel_name={tunnel_name}. Assigned tunnel_id={tunnel_id}");

                            return Ok(Tunnel::new(
                                tunnel_id,
                                tunnel_name.clone(),
                                ingress_id.clone(),
                                connection,
                                Some(client_byte_counter.clone())
                            ));
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
