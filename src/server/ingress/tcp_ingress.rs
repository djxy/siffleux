use crate::error::Error;
use crate::server::Server;
use crate::server::ingress::ingress::Ingress;
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Clone)]
pub struct TcpIngress {
    inner: Arc<TcpIngressInner>,
}

pub struct TcpIngressInner {
    port: u16,
}

#[async_trait]
impl Ingress for TcpIngress {
    async fn start(&self, server: &Server) -> Result<(), Error> {
        todo!()
    }

    async fn stop(&self, server: &Server) -> Result<(), Error> {
        todo!()
    }
}

impl TcpIngress {
    pub fn new(port: u16) -> Self {
        Self {
            inner: Arc::new(TcpIngressInner { port }),
        }
    }

    async fn listen(&self) {}
}
