use std::net::SocketAddr;

use base64::Engine;
use siffleux::{
    AuthKey, Ingress, IngressClone, IngressId, Server, TcpIngress, generate_self_signed_certificate,
};
use tracing::info;

use crate::{
    cli::{ServerArgs, TcpIngressAgrs},
    utils::{BASE64_ENGINE, generate_secure_random_key, wait_for_shutdown_signal},
};

pub async fn start_tcp_ingress(server_args: ServerArgs, tcp_args: TcpIngressAgrs) {
    let (cert_der, key, cert_hash) =
        generate_self_signed_certificate(&server_args.cert_subject_alt_name);

    let provided_auth_key = tcp_args.auth_key.is_some();
    let auth_key = tcp_args
        .auth_key
        .unwrap_or_else(|| AuthKey::try_from(generate_secure_random_key::<32>()).unwrap());
    let ingress_id = tcp_args
        .ingress_id
        .unwrap_or_else(|| IngressId::try_from(generate_secure_random_key::<16>()).unwrap());

    let server = Server::new_with_certificate(cert_der.clone(), key.clone_key()).unwrap();

    server
        .listen(SocketAddr::new(
            server_args.tunnel_ip,
            server_args.tunnel_port,
        ))
        .await
        .unwrap();

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        auth_key.hash(),
        SocketAddr::new(tcp_args.ip, tcp_args.port),
    );

    tcp_ingress.start().await.unwrap();

    server.assign_ingress(tcp_ingress.clone_box()).unwrap();

    info!(
        "Command to tunnel to your client:
siffleux client \\
    --server {} \\
    --cert-hash {} \\
    tcp \\
    --ingress-id {ingress_id} \\
    --auth-key {} \\
    --target <TARGET_IP>:<TARGET_PORT>",
        server.address().unwrap(),
        BASE64_ENGINE.encode(cert_hash),
        if provided_auth_key {
            "<AUTH_KEY>"
        } else {
            auth_key.to_str()
        },
    );

    wait_for_shutdown_signal().await;

    server.stop().await.unwrap();
}
