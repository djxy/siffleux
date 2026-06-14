use base64::Engine;
use siffleux::{Client, Egress, TcpEgress, TunnelName};

use crate::{
    cli::{ClientArgs, TcpEgressAgrs},
    utils::{BASE64_ENGINE, generate_secure_random_key, wait_for_shutdown_signal},
};

pub async fn start_tcp_egress(tunnel_args: ClientArgs, tcp_args: TcpEgressAgrs) {
    let client = Client::new();

    let (tunnel, endpoint) = client
        .connect_tunnel_with_certificate_hash(
            tcp_args.auth_key,
            tcp_args.ingress_id,
            tunnel_args.name.unwrap_or_else(|| {
                TunnelName::try_from(generate_secure_random_key::<16>()).unwrap()
            }),
            tunnel_args.server,
            tunnel_args.cert_subject_alt_name,
            BASE64_ENGINE.decode(tunnel_args.cert_hash).unwrap(),
        )
        .await
        .unwrap();

    let tcp_egress = TcpEgress::new(tunnel.clone(), tcp_args.target);

    tcp_egress.start().await.unwrap();

    wait_for_shutdown_signal().await;

    tcp_egress.stop().await.unwrap();

    tunnel.close().await;
    endpoint.wait_idle().await;
}
