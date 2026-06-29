use std::net::SocketAddr;

use siffleux::{Ingress, IngressClone, Server, TcpIngress};

use crate::{
    config::{
        IngressConfig::{self, TCP},
        ServerConfig, TcpIngressConfig,
    },
    utils::{load_or_generate_self_signed_certificate, wait_for_shutdown_signal},
};

pub async fn launch_server_with_ingresses(
    server_config: ServerConfig,
    ingress_configs: Vec<IngressConfig>,
) {
    let (cert_der, key, certificate_hash) =
        load_or_generate_self_signed_certificate(&server_config.cert_subject_alt_name).await;
    let server = Server::new_with_certificate(
        server_config.id,
        cert_der.clone(),
        key.clone_key(),
        certificate_hash,
    )
    .unwrap();

    server.listen(server_config.client_addr).await.unwrap();

    for ingress_config in ingress_configs {
        match ingress_config {
            TCP(tcp_ingress_config) => {
                let tcp_ingress = launch_tcp_ingress(tcp_ingress_config).await;

                server.assign_ingress(tcp_ingress.clone_box()).unwrap();
            }
        }
    }

    wait_for_shutdown_signal().await;

    server.stop().await.unwrap();
}

async fn launch_tcp_ingress(tcp_ingress_config: TcpIngressConfig) -> TcpIngress {
    let tcp_ingress = TcpIngress::new(
        tcp_ingress_config.ingress_id.clone(),
        tcp_ingress_config.auth_key.hash(),
        SocketAddr::new(tcp_ingress_config.ip, tcp_ingress_config.port),
    );

    tcp_ingress.start().await.unwrap();

    tcp_ingress
}
