use std::net::SocketAddr;

use base64::{Engine, engine::general_purpose};
use siffleux::{
    AuthKey, IngressId, Server, generate_self_signed_certificate, ingress::Ingress,
    tcp_ingress::TcpIngress,
};
use tracing::info;

use crate::cli::TcpIngressAgrs;

pub async fn start_tcp_ingress(tcp_args: TcpIngressAgrs) {
    let (cert_der, key, cert_hash) = generate_self_signed_certificate(SERVER_NAME);

    let provided_auth_key = tcp_args.auth_key.is_some();
    let auth_key = tcp_args
        .auth_key
        .unwrap_or_else(|| AuthKey::try_from(generate_secure_random_key()).unwrap());
    let ingress_id = tcp_args
        .ingress_id
        .unwrap_or_else(|| IngressId::try_from(generate_secure_random_key()).unwrap());

    let server =
        Server::new_with_certificate(auth_key.hash(), cert_der.clone(), key.clone_key()).unwrap();

    server
        .listen(SocketAddr::new(
            tcp_args.server_args.tunnel_ip,
            tcp_args.server_args.tunnel_port,
        ))
        .await
        .unwrap();

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        SocketAddr::new(tcp_args.ingress_ip, tcp_args.ingress_port),
    );

    tcp_ingress.start().await.unwrap();

    info!("Connect client");
    info!(
        "
        siffleux tunnel tcp \\
        --target-ip <TARGET_IP> --target-port <TARGET_PORT> \\
        --server-ip <SERVER_IP> --server-port <SERVER_PORT> \\
        --ingress-id {ingress_id} \\
        --auth-key {} \\
        --cert-hash {}
        ",
        if provided_auth_key {
            "<AUTH_KEY>"
        } else {
            auth_key.to_str()
        },
        general_purpose::URL_SAFE.encode(cert_hash)
    );
}
