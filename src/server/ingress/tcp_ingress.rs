use crate::common::tunnel::Tunnel;
use crate::common::types::IngressId;
use crate::server::ingress::ingress::Ingress;
use crate::server::server::Server;
use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

pub struct TcpIngress {
    id: IngressId,
    tasks: RwLock<Vec<JoinHandle<()>>>,
}

#[async_trait]
impl Ingress for TcpIngress {
    fn id(&self) -> &IngressId {
        &self.id
    }

    fn assign_tunnel(&self, _tunnel: Tunnel) {}

    async fn start(&self, _server: &Server) {
        let mut tasks = self.tasks.write().await;

        if !tasks.is_empty() {
            return;
        }

        tasks.push(self.start_listen());
    }

    async fn stop(&self, _server: &Server) {
        let mut tasks = self.tasks.write().await;

        tasks.iter().for_each(|t| t.abort());

        *tasks = vec![];
    }
}

impl TcpIngress {
    pub fn new(id: IngressId) -> Self {
        Self {
            id,
            tasks: RwLock::new(Vec::new()),
        }
    }

    fn start_listen(&self) -> JoinHandle<()> {
        tokio::spawn(async move {
            // Starting TCP socket
        })
    }
}
