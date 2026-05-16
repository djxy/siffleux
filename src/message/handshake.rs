use crate::error::Error;
use crate::types::{AuthKey, IngressId, TunnelName};
use bytes::{BufMut, BytesMut};
use quinn::{RecvStream, SendStream};
use uuid::Uuid;

const VERSION: u8 = 1;

#[derive(Debug)]
pub struct HandshakeV1Request {
    pub auth_key: String,
    pub ingress_id: String,
    pub name: String,
}

#[derive(Debug)]
pub struct HandshakeV1Response {
    pub id: Uuid,
}

impl HandshakeV1Request {
    pub async fn write(
        send: &mut SendStream,
        auth_key: &AuthKey,
        ingress_id: &IngressId,
        tunnel_name: &TunnelName,
    ) -> Result<(), Error> {
        let mut buffer = BytesMut::with_capacity(
            (1 + 1 + auth_key.len() + 1 + ingress_id.len() + 1 + tunnel_name.len()) as usize,
        );

        buffer.put_u8(VERSION);
        buffer.put_u8(auth_key.len());
        buffer.put_slice(auth_key.value().as_bytes());
        buffer.put_u8(ingress_id.len());
        buffer.put_slice(ingress_id.value().as_bytes());
        buffer.put_u8(tunnel_name.len());
        buffer.put_slice(tunnel_name.value().as_bytes());

        send.write_chunk(buffer.freeze()).await?;

        Ok(())
    }

    pub async fn read(recv: &mut RecvStream) -> Result<HandshakeV1Request, Error> {
        let mut buffer = [0u8; 255];
        recv.read_exact(&mut buffer[..1]).await?;
        let version = buffer[0];

        if version != VERSION {
            return Err(Error::IncompatibleVersion {
                expected: VERSION,
                received: version,
            });
        }

        recv.read_exact(&mut buffer[..1]).await?;
        let auth_key_len = buffer[0] as usize;

        recv.read_exact(&mut buffer[..auth_key_len]).await?;

        let auth_key = String::from_utf8(buffer[..auth_key_len].to_vec())?;

        recv.read_exact(&mut buffer[..1]).await?;
        let ingress_id_len = buffer[0] as usize;

        recv.read_exact(&mut buffer[..ingress_id_len]).await?;

        let ingress_id = String::from_utf8(buffer[..ingress_id_len].to_vec())?;

        recv.read_exact(&mut buffer[..1]).await?;
        let name_len = buffer[0] as usize;

        recv.read_exact(&mut buffer[..name_len]).await?;

        let name = String::from_utf8(buffer[..name_len].to_vec())?;

        Ok(HandshakeV1Request::new(auth_key, ingress_id, name))
    }

    pub fn new(auth_key: String, ingress_id: String, name: String) -> HandshakeV1Request {
        HandshakeV1Request {
            auth_key,
            ingress_id,
            name,
        }
    }
}

impl HandshakeV1Response {
    pub async fn write(send: &mut SendStream, id: Uuid) -> Result<(), Error> {
        send.write_all(id.as_bytes()).await?;

        Ok(())
    }

    pub async fn read(recv: &mut RecvStream) -> Result<HandshakeV1Response, Error> {
        let mut buffer = [0u8; 16];

        recv.read_exact(&mut buffer[..]).await?;

        Ok(HandshakeV1Response::new(Uuid::from_slice(&buffer)?))
    }

    pub fn new(id: Uuid) -> HandshakeV1Response {
        HandshakeV1Response { id }
    }
}
