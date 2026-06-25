use std::net::SocketAddr;

use siffleux::{AuthKey, Ingress, IngressClone, IngressId, Server, TcpIngress};

use crate::{
    config::{
        IngressConfig::{self, TCP},
        ServerConfig, TcpIngressConfig,
    },
    utils::{
        generate_secure_random_key, load_or_generate_self_signed_certificate,
        wait_for_shutdown_signal,
    },
};

pub async fn launch_server_with_ingresses(
    server_config: ServerConfig,
    ingress_configs: Vec<IngressConfig>,
) {
    let (cert_der, key, certificate_hash) =
        load_or_generate_self_signed_certificate(&server_config.cert_subject_alt_name).await;
    let server =
        Server::new_with_certificate(cert_der.clone(), key.clone_key(), certificate_hash).unwrap();

    server
        .listen(SocketAddr::new(
            server_config.tunnel_ip,
            server_config.tunnel_port,
        ))
        .await
        .unwrap();

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
    let auth_key = tcp_ingress_config
        .auth_key
        .unwrap_or_else(|| AuthKey::try_from(generate_secure_random_key::<32>()).unwrap());
    let ingress_id = tcp_ingress_config
        .ingress_id
        .unwrap_or_else(|| IngressId::try_from(generate_secure_random_key::<16>()).unwrap());

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        auth_key.hash(),
        SocketAddr::new(tcp_ingress_config.ip, tcp_ingress_config.port),
    );

    tcp_ingress.start().await.unwrap();

    tcp_ingress
}
