use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use uuid::Uuid;

use crate::{Error, IngressId, ServerId, common::AuthKey};

pub const VERSION: &[u8] = b"siffleux/v1";

const AUTH_TYPE: u8 = 0;
const AUTHENTICATED_TYPE: u8 = 1;
const PING_TYPE: u8 = 3;
const PONG_TYPE: u8 = 4;

pub struct CodecV1;

pub enum FrameV1 {
    Auth {
        auth_key: AuthKey,
        ingress_id: IngressId,
    },
    Authenticated {
        tunnel_id: Uuid,
        server_id: ServerId,
    },
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
            } => {
                let auth_key_str = auth_key.to_str();
                let ingress_id_str = ingress_id.to_str();

                let payload_length = (1 + auth_key_str.len()) + (1 + ingress_id_str.len());

                dst.reserve(1 + 2 + payload_length);

                dst.put_u8(AUTH_TYPE);
                dst.put_u16(payload_length as u16);

                dst.put_u8(auth_key_str.len() as u8);
                dst.put_slice(auth_key_str.as_bytes());

                dst.put_u8(ingress_id_str.len() as u8);
                dst.put_slice(ingress_id_str.as_bytes());
            }
            FrameV1::Authenticated {
                tunnel_id,
                server_id,
            } => {
                let server_id_str = server_id.to_str();

                let payload_length = (1 + server_id_str.len()) + 16;

                dst.reserve(1 + 2 + payload_length);

                dst.put_u8(AUTHENTICATED_TYPE);
                dst.put_u16(payload_length as u16);

                dst.put_u8(server_id_str.len() as u8);
                dst.put_slice(server_id_str.as_bytes());

                dst.put_slice(tunnel_id.as_bytes());
            }
            FrameV1::Ping => {
                dst.reserve(1);
                dst.put_u8(PING_TYPE);
            }
            FrameV1::Pong => {
                dst.reserve(1);
                dst.put_u8(PONG_TYPE);
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

                return Ok(Some(FrameV1::Auth {
                    auth_key: AuthKey::from_bytes(&auth_key_bytes)?,
                    ingress_id: IngressId::from_bytes(&ingress_id_bytes)?,
                }));
            }
            AUTHENTICATED_TYPE => {
                let payload_length = u16::from_be_bytes([src[1], src[2]]) as usize;

                if src_len < 1 + 2 + payload_length {
                    return Ok(None);
                }

                src.advance(3);

                let server_id_len = src.get_u8();
                let server_id_bytes = src.split_to(server_id_len as usize);
                let tunnel_id_bytes = src.split_to(16);

                return Ok(Some(FrameV1::Authenticated {
                    server_id: ServerId::from_bytes(&server_id_bytes)?,
                    tunnel_id: Uuid::from_slice(&tunnel_id_bytes)?,
                }));
            }
            PING_TYPE => {
                src.advance(1);
                return Ok(Some(FrameV1::Ping));
            }
            PONG_TYPE => {
                src.advance(1);
                return Ok(Some(FrameV1::Pong));
            }
            _ => {}
        }

        Ok(None)
    }
}

pub const UDP_IPV4_ORIGIN: u8 = 0;
pub const UDP_IPV6_ORIGIN: u8 = 1;

const UDP_HEADER_IPV4_LEN: usize = 1 + 4 + 2; // IP version + IP + port
const UDP_HEADER_IPV6_LEN: usize = 1 + 16 + 2; // IP version + IP + port
const HEADER_GAP_IPV4: usize = UDP_HEADER_IPV6_LEN - UDP_HEADER_IPV4_LEN;

const UDP_PAYLOAD_CAPACITY: usize = 1200;

const UDP_DEFAULT_BYTES_CAPACITY: usize = UDP_HEADER_IPV6_LEN + UDP_PAYLOAD_CAPACITY;

pub struct UdpMessage {
    bytes: Option<BytesMut>,
}

impl UdpMessage {
    fn extract_origin_addr(udp_message_bytes: &mut Bytes) -> Option<SocketAddr> {
        match udp_message_bytes.get_u8() {
            UDP_IPV4_ORIGIN => {
                let mut octets = [0u8; 4];

                udp_message_bytes.copy_to_slice(&mut octets);

                Some(SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::from(octets),
                    udp_message_bytes.get_u16(),
                )))
            }
            UDP_IPV6_ORIGIN => {
                let mut octets = [0u8; 16];

                udp_message_bytes.copy_to_slice(&mut octets);

                Some(SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::from(octets),
                    udp_message_bytes.get_u16(),
                    0,
                    0,
                )))
            }
            _ => None,
        }
    }

    pub fn new() -> Self {
        let mut bytes = BytesMut::with_capacity(UDP_DEFAULT_BYTES_CAPACITY);

        bytes.resize(UDP_DEFAULT_BYTES_CAPACITY, 0);

        Self { bytes: Some(bytes) }
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.bytes.as_mut().unwrap()[UDP_HEADER_IPV6_LEN..]
    }

    pub fn to_datagram(&mut self, origin_addr: SocketAddr, payload_len: usize) -> Bytes {
        let mut bytes = self.bytes.take().unwrap();

        bytes.truncate(UDP_HEADER_IPV6_LEN + payload_len);

        match origin_addr.ip() {
            IpAddr::V4(v4) => {
                let header = &mut bytes[HEADER_GAP_IPV4..UDP_HEADER_IPV6_LEN];

                header[0] = UDP_IPV4_ORIGIN;
                header[1..5].copy_from_slice(&v4.octets());
                header[5..7].copy_from_slice(&origin_addr.port().to_be_bytes());

                bytes.advance(HEADER_GAP_IPV4);
            }
            IpAddr::V6(v6) => {
                let header = &mut bytes[..UDP_HEADER_IPV6_LEN];

                header[0] = UDP_IPV6_ORIGIN;
                header[1..17].copy_from_slice(&v6.octets());
                header[17..19].copy_from_slice(&origin_addr.port().to_be_bytes());
            }
        }

        bytes.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_mut_has_full_spare_capacity() {
        let mut msg = UdpMessage::new();
        assert_eq!(msg.payload_mut().len(), UDP_PAYLOAD_CAPACITY);
    }

    #[test]
    fn payload_mut_is_writable_and_isolated_from_header_region() {
        let mut msg = UdpMessage::new();
        let payload = msg.payload_mut();
        payload[0] = 0xAA;
        payload[1] = 0xBB;

        let origin: SocketAddr = "203.0.113.5:4242".parse().unwrap();
        let datagram = msg.to_datagram(origin, 2);

        assert_eq!(datagram.len(), UDP_HEADER_IPV4_LEN + 2);
        assert_eq!(datagram[UDP_HEADER_IPV4_LEN], 0xAA);
        assert_eq!(datagram[UDP_HEADER_IPV4_LEN + 1], 0xBB);
    }

    #[test]
    fn ipv4_header_layout_and_length() {
        let mut msg = UdpMessage::new();
        let payload_data = b"hello";
        msg.payload_mut()[..payload_data.len()].copy_from_slice(payload_data);

        let origin: SocketAddr = "192.168.1.42:5000".parse().unwrap();
        let datagram = msg.to_datagram(origin, payload_data.len());

        assert_eq!(datagram.len(), UDP_HEADER_IPV4_LEN + payload_data.len());
        assert_eq!(datagram[0], UDP_IPV4_ORIGIN);
        assert_eq!(&datagram[1..5], &Ipv4Addr::new(192, 168, 1, 42).octets());
        assert_eq!(&datagram[5..7], &5000u16.to_be_bytes());
        assert_eq!(&datagram[UDP_HEADER_IPV4_LEN..], payload_data);
    }

    #[test]
    fn ipv6_header_layout_and_length() {
        let mut msg = UdpMessage::new();
        let payload_data = b"world!";
        msg.payload_mut()[..payload_data.len()].copy_from_slice(payload_data);

        let ip = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1);
        let origin: SocketAddr = SocketAddr::new(IpAddr::V6(ip), 51820);
        let datagram = msg.to_datagram(origin, payload_data.len());

        assert_eq!(datagram.len(), UDP_HEADER_IPV6_LEN + payload_data.len());
        assert_eq!(datagram[0], UDP_IPV6_ORIGIN);
        assert_eq!(&datagram[1..17], &ip.octets());
        assert_eq!(&datagram[17..19], &51820u16.to_be_bytes());
        assert_eq!(&datagram[UDP_HEADER_IPV6_LEN..], payload_data);
    }

    #[test]
    fn zero_length_payload_ipv4() {
        let mut msg = UdpMessage::new();
        let origin: SocketAddr = "10.0.0.1:1".parse().unwrap();
        let datagram = msg.to_datagram(origin, 0);
        assert_eq!(datagram.len(), UDP_HEADER_IPV4_LEN);
        assert_eq!(datagram[0], UDP_IPV4_ORIGIN);
    }

    #[test]
    fn zero_length_payload_ipv6() {
        let mut msg = UdpMessage::new();
        let origin: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 1);
        let datagram = msg.to_datagram(origin, 0);
        assert_eq!(datagram.len(), UDP_HEADER_IPV6_LEN);
        assert_eq!(datagram[0], UDP_IPV6_ORIGIN);
    }

    #[test]
    fn max_capacity_payload_ipv4() {
        let mut msg = UdpMessage::new();
        let fill: Vec<u8> = (0..UDP_PAYLOAD_CAPACITY).map(|i| (i % 256) as u8).collect();
        msg.payload_mut().copy_from_slice(&fill);

        let origin: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let datagram = msg.to_datagram(origin, UDP_PAYLOAD_CAPACITY);

        assert_eq!(datagram.len(), UDP_HEADER_IPV4_LEN + UDP_PAYLOAD_CAPACITY);
        assert_eq!(&datagram[UDP_HEADER_IPV4_LEN..], &fill[..]);
    }

    #[test]
    fn max_capacity_payload_ipv6() {
        let mut msg = UdpMessage::new();
        let fill: Vec<u8> = (0..UDP_PAYLOAD_CAPACITY).map(|i| (i % 256) as u8).collect();
        msg.payload_mut().copy_from_slice(&fill);

        let origin: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9999);
        let datagram = msg.to_datagram(origin, UDP_PAYLOAD_CAPACITY);

        assert_eq!(datagram.len(), UDP_HEADER_IPV6_LEN + UDP_PAYLOAD_CAPACITY);
        assert_eq!(&datagram[UDP_HEADER_IPV6_LEN..], &fill[..]);
    }

    #[test]
    fn ipv4_and_ipv6_headers_have_expected_length_difference() {
        assert_eq!(UDP_HEADER_IPV6_LEN - UDP_HEADER_IPV4_LEN, HEADER_GAP_IPV4);
        assert_eq!(UDP_HEADER_IPV4_LEN, 7);
        assert_eq!(UDP_HEADER_IPV6_LEN, 19);
    }

    #[test]
    fn different_messages_do_not_share_state() {
        let mut msg1 = UdpMessage::new();
        let mut msg2 = UdpMessage::new();

        msg1.payload_mut()[0] = 1;
        msg2.payload_mut()[0] = 2;

        let origin: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let d1 = msg1.to_datagram(origin, 1);
        let d2 = msg2.to_datagram(origin, 1);

        assert_eq!(d1[UDP_HEADER_IPV4_LEN], 1);
        assert_eq!(d2[UDP_HEADER_IPV4_LEN], 2);
    }

    #[test]
    fn extract_origin_addr_ipv4_zero_length_payload() {
        let origin: SocketAddr = "10.0.0.1:1".parse().unwrap();
        let mut msg = UdpMessage::new();
        let mut datagram = msg.to_datagram(origin, 0);

        let extracted = UdpMessage::extract_origin_addr(&mut datagram);

        assert_eq!(extracted, Some(origin));
        assert!(datagram.is_empty());
    }

    #[test]
    fn extract_origin_addr_ipv6_zero_length_payload() {
        let origin: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 1);
        let mut msg = UdpMessage::new();
        let mut datagram = msg.to_datagram(origin, 0);

        let extracted = UdpMessage::extract_origin_addr(&mut datagram);

        assert_eq!(extracted, Some(origin));
        assert!(datagram.is_empty());
    }

    #[test]
    fn extract_origin_addr_ipv4_port_zero_and_max() {
        for port in [0u16, u16::MAX] {
            let origin: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), port);
            let mut msg = UdpMessage::new();
            let mut datagram = msg.to_datagram(origin, 0);

            assert_eq!(UdpMessage::extract_origin_addr(&mut datagram), Some(origin));
        }
    }

    #[test]
    fn extract_origin_addr_ipv6_port_zero_and_max() {
        for port in [0u16, u16::MAX] {
            let origin: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port);
            let mut msg = UdpMessage::new();
            let mut datagram = msg.to_datagram(origin, 0);

            assert_eq!(UdpMessage::extract_origin_addr(&mut datagram), Some(origin));
        }
    }

    #[test]
    fn extract_origin_addr_invalid_tag_returns_none() {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&[0xFF, 1, 2, 3, 4, 5, 6]);
        let mut bytes = buf.freeze();

        let extracted = UdpMessage::extract_origin_addr(&mut bytes);

        assert_eq!(extracted, None);
        assert_eq!(&bytes[..], &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn extract_origin_addr_empty_buffer_panics() {
        let mut bytes = Bytes::new();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                UdpMessage::extract_origin_addr(&mut bytes)
            }))
            .is_err()
        );
    }

    #[test]
    fn extract_origin_addr_ipv4_roundtrip() {
        let origin: SocketAddr = "192.168.1.42:5000".parse().unwrap();
        let mut msg = UdpMessage::new();
        let payload_data = b"hello";
        msg.payload_mut()[..payload_data.len()].copy_from_slice(payload_data);

        let mut datagram = msg.to_datagram(origin, payload_data.len());
        let extracted = UdpMessage::extract_origin_addr(&mut datagram);

        assert_eq!(extracted, Some(origin));
        assert_eq!(&datagram[..], payload_data);
    }

    #[test]
    fn extract_origin_addr_ipv6_roundtrip() {
        let ip = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1);
        let origin: SocketAddr = SocketAddr::new(IpAddr::V6(ip), 51820);
        let mut msg = UdpMessage::new();
        let payload_data = b"world!";
        msg.payload_mut()[..payload_data.len()].copy_from_slice(payload_data);

        let mut datagram = msg.to_datagram(origin, payload_data.len());
        let extracted = UdpMessage::extract_origin_addr(&mut datagram);

        assert_eq!(extracted, Some(origin));
        assert_eq!(&datagram[..], payload_data);
    }

    #[test]
    fn full_pipeline_to_datagram_then_extract_preserves_payload_bytes() {
        let origin: SocketAddr = "203.0.113.5:4242".parse().unwrap();
        let mut msg = UdpMessage::new();
        let fill: Vec<u8> = (0..UDP_PAYLOAD_CAPACITY).map(|i| (i % 256) as u8).collect();
        msg.payload_mut().copy_from_slice(&fill);

        let mut datagram = msg.to_datagram(origin, UDP_PAYLOAD_CAPACITY);
        let extracted = UdpMessage::extract_origin_addr(&mut datagram);

        assert_eq!(extracted, Some(origin));
        assert_eq!(&datagram[..], &fill[..]);
    }
}
