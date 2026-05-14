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
        auth_key: &str,
        ingress_id: &str,
        name: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(auth_key.len() <= 255, "auth_key too long. Max 255 bytes.");
        anyhow::ensure!(
            ingress_id.len() <= 255,
            "ingress_id too long. Max 255 bytes."
        );
        anyhow::ensure!(name.len() <= 255, "name too long. Max 255 bytes.");

        let mut buffer =
            BytesMut::with_capacity(1 + 1 + auth_key.len() + 1 + ingress_id.len() + 1 + name.len());

        buffer.put_u8(VERSION);
        buffer.put_u8(auth_key.len() as u8);
        buffer.put_slice(auth_key.as_bytes());
        buffer.put_u8(ingress_id.len() as u8);
        buffer.put_slice(ingress_id.as_bytes());
        buffer.put_u8(name.len() as u8);
        buffer.put_slice(name.as_bytes());

        send.write_chunk(buffer.freeze()).await?;

        Ok(())
    }

    pub async fn read(recv: &mut RecvStream) -> anyhow::Result<HandshakeV1Request> {
        let mut buffer = [0u8; 255];
        recv.read_exact(&mut buffer[..1]).await?;
        let version = buffer[0];

        anyhow::ensure!(VERSION == version, "Incompatible version received.");

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
    pub async fn write(send: &mut SendStream, id: Uuid) -> anyhow::Result<()> {
        send.write_all(id.as_bytes()).await?;

        Ok(())
    }

    pub async fn read(recv: &mut RecvStream) -> anyhow::Result<HandshakeV1Response> {
        let mut buffer = [0u8; 16];

        recv.read_exact(&mut buffer[..]).await?;

        Ok(HandshakeV1Response::new(Uuid::from_slice(&buffer)?))
    }

    pub fn new(id: Uuid) -> HandshakeV1Response {
        HandshakeV1Response { id }
    }
}
