use quinn::{RecvStream, SendStream};
use uuid::Uuid;

pub struct TunnelStream {
    tunnel_id: Uuid,
    send_stream: SendStream,
    receive_stream: RecvStream,
}

impl TunnelStream {
    pub fn new(send_stream: SendStream, receive_stream: RecvStream) -> TunnelStream {
        TunnelStream {
            tunnel_id: Uuid::new_v4(),
            send_stream,
            receive_stream,
        }
    }

    pub async fn send(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        self.send_stream.write_all(&bytes).await?;

        Ok(())
    }

    pub async fn read(&mut self, bytes: &mut [u8]) -> anyhow::Result<()> {
        self.receive_stream.read_exact(bytes).await?;

        Ok(())
    }
}
