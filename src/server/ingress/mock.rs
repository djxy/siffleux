use crate::error::Error;
use crate::server::ingress::ingress::Ingress;
use crate::server::server_tunnel::ServerTunnel;
use crate::server::Server;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct MockIngress {
    inner: Arc<MockIngressInner>,
}

pub struct MockIngressInner {
    tasks: RwLock<Vec<JoinHandle<()>>>,
    pub tunnels_connected: RwLock<Vec<ServerTunnel>>,
}

impl Default for MockIngress {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Ingress for MockIngress {
    async fn start(&self, server: &Server) -> Result<(), Error> {
        let mut tasks = self.inner.tasks.write().await;

        tasks.push(self.listen_on_tunnel_connected(server));

        Ok(())
    }

    async fn stop(&self, server: &Server) -> Result<(), Error> {
        let mut tasks = self.inner.tasks.write().await;

        tasks.iter().for_each(|task| task.abort());

        tasks.clear();

        Ok(())
    }
}

impl MockIngress {
    pub fn new() -> MockIngress {
        MockIngress {
            inner: Arc::new(MockIngressInner {
                tasks: RwLock::new(Vec::new()),
                tunnels_connected: RwLock::new(Vec::new()),
            }),
        }
    }

    fn listen_on_tunnel_connected(&self, server: &Server) -> JoinHandle<()> {
        let mut on_tunnel_connected = server.subscribe_on_tunnel_connected();
        let self_clone = self.clone();

        tokio::spawn(async move {
            while let Ok(tunnel) = on_tunnel_connected.recv().await {
                self_clone.inner.tunnels_connected.write().await.push(tunnel);
            }
        })
    }
}
