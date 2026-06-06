use bytes::{BufMut, BytesMut};
use quinn::{RecvStream, SendStream};

use crate::{AuthKey, Error, IngressId, TunnelId, TunnelName};

#[derive(Debug)]
pub struct Request {
    pub auth_key: AuthKey,
    pub ingress_id: IngressId,
    pub tunnel_name: TunnelName,
}

#[derive(Debug)]
pub struct Response {
    pub tunnel_id: TunnelId,
}

impl Request {
    pub async fn write(
        send: &mut SendStream,
        auth_key: &AuthKey,
        ingress_id: &IngressId,
        tunnel_name: &TunnelName,
    ) -> Result<(), Error> {
        let auth_key_len = auth_key.to_str().len() as u8;
        let mut buffer = BytesMut::with_capacity(
            (
                // Auth key
                1 + auth_key_len +
                // Ingress ID
                1 + ingress_id.len() +
                // Tunnel name
                1 + tunnel_name.len()
            ) as usize,
        );

        buffer.put_u8(auth_key_len);
        buffer.put_slice(auth_key.to_str().as_bytes());
        buffer.put_u8(ingress_id.len());
        buffer.put_slice(ingress_id.value().as_bytes());
        buffer.put_u8(tunnel_name.len());
        buffer.put_slice(tunnel_name.value().as_bytes());

        send.write_chunk(buffer.freeze()).await?;

        Ok(())
    }

    pub async fn read(recv: &mut RecvStream) -> Result<Request, Error> {
        let mut buffer = [0u8; 255];

        recv.read_exact(&mut buffer[..1]).await?;
        let auth_key_len = buffer[0] as usize;

        recv.read_exact(&mut buffer[..auth_key_len]).await?;

        let auth_key = String::from_utf8(buffer[..auth_key_len].to_vec())?;

        recv.read_exact(&mut buffer[..1]).await?;
        let ingress_id_len = buffer[0] as usize;

        recv.read_exact(&mut buffer[..ingress_id_len]).await?;

        let ingress_id = String::from_utf8(buffer[..ingress_id_len].to_vec())?;

        recv.read_exact(&mut buffer[..1]).await?;
        let tunnel_name_len = buffer[0] as usize;

        recv.read_exact(&mut buffer[..tunnel_name_len]).await?;

        let tunnel_name = String::from_utf8(buffer[..tunnel_name_len].to_vec())?;

        Ok(Request::new(
            AuthKey::try_from(auth_key)?,
            IngressId::try_from(ingress_id)?,
            TunnelName::try_from(tunnel_name)?,
        ))
    }

    pub fn new(auth_key: AuthKey, ingress_id: IngressId, tunnel_name: TunnelName) -> Self {
        Self {
            auth_key,
            ingress_id,
            tunnel_name,
        }
    }
}

impl Response {
    pub async fn write(send: &mut SendStream, id: TunnelId) -> Result<(), Error> {
        send.write_all(&id.to_bytes()).await?;

        Ok(())
    }

    pub async fn read(recv: &mut RecvStream) -> Result<Response, Error> {
        let mut buffer = [0u8; 8];

        recv.read_exact(&mut buffer[..]).await?;

        Ok(Response::new(TunnelId::from_bytes(buffer)))
    }

    pub fn new(tunnel_id: TunnelId) -> Self {
        Self { tunnel_id }
    }
}
