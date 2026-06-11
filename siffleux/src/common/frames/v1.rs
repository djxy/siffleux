use bytes::{Buf, BufMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::{
    Error,
    common::{AuthKey, IngressId, TunnelId, TunnelName},
};

pub const VERSION: &[u8] = b"siffleux/v1";

const AUTH_TYPE: u8 = 0;
const AUTHENTICATED_TYPE: u8 = 1;
const TCP_STREAM_TYPE: u8 = 2;
const PING_TYPE: u8 = 3;
const PONG_TYPE: u8 = 4;

pub struct CodecV1;

pub enum FrameV1 {
    Auth {
        auth_key: AuthKey,
        ingress_id: IngressId,
        tunnel_name: TunnelName,
    },
    Authenticated {
        tunnel_id: TunnelId,
    },
    TCPStream,
    Ping,
    Pong,
}

impl Encoder<FrameV1> for CodecV1 {
    type Error = Error;

    fn encode(&mut self, item: FrameV1, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        match item {
            FrameV1::Auth {
                auth_key,
                ingress_id,
                tunnel_name,
            } => {
                let auth_key_str = auth_key.to_str();
                let ingress_id_str = ingress_id.to_str();
                let tunnel_name_str = tunnel_name.to_str();

                let payload_length = (1 + auth_key_str.len())
                    + (1 + ingress_id_str.len())
                    + (1 + tunnel_name_str.len());

                dst.reserve(1 + 2 + payload_length);

                dst.put_u8(AUTH_TYPE);
                dst.put_u16(payload_length as u16);

                dst.put_u8(auth_key_str.len() as u8);
                dst.put_slice(auth_key_str.as_bytes());

                dst.put_u8(ingress_id_str.len() as u8);
                dst.put_slice(ingress_id_str.as_bytes());

                dst.put_u8(tunnel_name_str.len() as u8);
                dst.put_slice(tunnel_name_str.as_bytes());
            }
            FrameV1::Authenticated { tunnel_id } => {
                dst.reserve(1 + 8);
                dst.put_u8(AUTHENTICATED_TYPE);
                dst.put_slice(&tunnel_id.to_bytes());
            }
            FrameV1::Ping => {
                dst.reserve(1);
                dst.put_u8(PING_TYPE);
            }
            FrameV1::Pong => {
                dst.reserve(1);
                dst.put_u8(PONG_TYPE);
            }
            FrameV1::TCPStream => {
                dst.reserve(1);
                dst.put_u8(TCP_STREAM_TYPE);
            }
        }

        Ok(())
    }
}

impl Decoder for CodecV1 {
    type Item = FrameV1;

    type Error = Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let src_len = src.len();

        if src_len == 0 {
            return Ok(None);
        }

        let frame_type = src[0];

        match frame_type {
            AUTH_TYPE => {
                let payload_length = u16::from_be_bytes([src[1], src[2]]) as usize;

                if src_len < 1 + 2 + payload_length {
                    return Ok(None);
                }

                src.advance(3);

                let auth_key_len = src.get_u8();
                let auth_key_bytes = src.split_to(auth_key_len as usize);
                let ingress_id_len = src.get_u8();
                let ingress_id_bytes = src.split_to(ingress_id_len as usize);
                let tunnel_name_len = src.get_u8();
                let tunnel_name_bytes = src.split_to(tunnel_name_len as usize);

                return Ok(Some(FrameV1::Auth {
                    auth_key: AuthKey::from_bytes(&auth_key_bytes)?,
                    ingress_id: IngressId::from_bytes(&ingress_id_bytes)?,
                    tunnel_name: TunnelName::from_bytes(&tunnel_name_bytes)?,
                }));
            }
            AUTHENTICATED_TYPE => {
                if src_len < 9 {
                    return Ok(None);
                }

                src.advance(1);

                let tunnel_id = TunnelId::new(src.get_u64());

                return Ok(Some(FrameV1::Authenticated { tunnel_id }));
            }
            PING_TYPE => {
                src.advance(1);
                return Ok(Some(FrameV1::Ping));
            }
            PONG_TYPE => {
                src.advance(1);
                return Ok(Some(FrameV1::Pong));
            }
            TCP_STREAM_TYPE => {
                src.advance(1);
                return Ok(Some(FrameV1::TCPStream));
            }
            _ => {}
        }

        Ok(None)
    }
}
