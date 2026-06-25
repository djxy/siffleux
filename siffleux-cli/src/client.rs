use std::net::SocketAddr;

use base64::Engine;
use siffleux::{Client, Egress, TcpEgress, Tunnel, TunnelName};
use tokio::net::lookup_host;
use tracing::error;

use crate::{
    config::{
        EgressConfig::{self, TCP},
        TcpEgressConfig, TunnelConfig,
    },
    utils::{BASE64_ENGINE, generate_secure_random_key, wait_for_shutdown_signal},
};

pub async fn launch_client_with_egresses(egress_configs: Vec<EgressConfig>) {
    let client = Client::new();
    let tunnels

    for egress_config in egress_configs {
        match egress_config {
            TCP(tcp_egress_config) => match launch_tcp_egress(&client, &tcp_egress_config).await {
                Ok(tunnel) => {}
                Err(e) => {
                    error!(
                        ingress_id = %tcp_egress_config.ingress_id.clone(),
                        e
                    );
                    return;
                }
            },
        }
    }

    wait_for_shutdown_signal().await;
    info!("Closing...");

    tcp_egress.stop().await.unwrap();
    tunnel.close().await;
    endpoint.wait_idle().await;

    info!("Closed");
}

async fn launch_tcp_egress(
    client: &Client,
    tcp_egress_config: &TcpEgressConfig,
) -> Result<Tunnel, String> {
    let (server_address, tunnel_name, certificate_hash) =
        prepare_tunnel(&tcp_egress_config.tunnel_config).await?;

    let tunnel = match client
        .connect_tunnel_with_certificate_hash(
            tcp_egress_config.auth_key.clone(),
            tcp_egress_config.ingress_id.clone(),
            tunnel_name.clone(),
            server_address,
            tcp_egress_config
                .tunnel_config
                .cert_subject_alt_name
                .clone(),
            certificate_hash,
        )
        .await
    {
        Ok(tunnel) => tunnel,
        Err(e) => {
            return Err(format!("Error while connecting to server: {e}"));
        }
    };

    let tcp_egress = TcpEgress::new(tunnel.clone(), tcp_egress_config.target);

    tcp_egress.start().await.unwrap();

    Ok(tunnel)

    // tokio::select! {
    //     _ = tunnel.closed() => {
    //         warn!(
    //             server_address = %server_address,
    //             ingress_id = %tcp_args.egress_args.ingress_id.clone(),
    //             "Server disconnected, reconnecting in 5 seconds."
    //         );

    //         sleep(Duration::from_secs(5)).await;
    //     }
    //     _ = wait_for_shutdown_signal() => {
    //         info!("Closing...");

    //         tcp_egress.stop().await.unwrap();
    //         tunnel.close().await;
    //         endpoint.wait_idle().await;

    //         info!("Closed");

    //         break;
    //     }

    // }
}

async fn prepare_tunnel(
    tunnel_config: &TunnelConfig,
) -> Result<(SocketAddr, TunnelName, Vec<u8>), String> {
    let Some(server_address) = lookup_host(&tunnel_config.server)
        .await
        .map_or_else(|_| None, |mut a| a.next().or(None))
    else {
        return Err(format!("Invalid server address: {}", &tunnel_config.server));
    };

    let tunnel_name = tunnel_config
        .name
        .clone()
        .unwrap_or_else(|| TunnelName::try_from(generate_secure_random_key::<16>()).unwrap());
    let certificate_hash = BASE64_ENGINE
        .decode(tunnel_config.cert_hash.clone())
        .unwrap();

    Ok((server_address, tunnel_name, certificate_hash))
}
