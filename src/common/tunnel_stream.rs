use quinn::{RecvStream, SendStream};

pub struct TunnelStream {
    send_stream: SendStream,
    recv_stream: RecvStream,
}

impl TunnelStream {
    pub fn new(send_stream: SendStream, recv_stream: RecvStream) -> TunnelStream {
        TunnelStream {
            send_stream,
            recv_stream,
        }
    }
}
