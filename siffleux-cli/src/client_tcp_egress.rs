use std::time::Duration;

use base64::Engine;
use siffleux::{Client, Egress, TcpEgress, TunnelName};
use tokio::{net::lookup_host, time::sleep};
use tracing::{error, info, warn};

use crate::{
    cli::{ClientArgs, TcpEgressAgrs},
    utils::{BASE64_ENGINE, generate_secure_random_key, wait_for_shutdown_signal},
};

pub async fn start_tcp_egress(client_args: ClientArgs, tcp_args: TcpEgressAgrs) {
    let Some(server_address) = lookup_host(&client_args.server)
        .await
        .map_or_else(|_| None, |mut a| a.next().or(None))
    else {
        error!("Invalid server address: {}", &client_args.server);
        return;
    };

    let client = Client::new();

    let tunnel_name = client_args
        .name
        .unwrap_or_else(|| TunnelName::try_from(generate_secure_random_key::<16>()).unwrap());
    let certificate_hash = BASE64_ENGINE.decode(client_args.cert_hash).unwrap();

    loop {
        let (tunnel, endpoint) = match client
            .connect_tunnel_with_certificate_hash(
                tcp_args.egress_args.auth_key.clone(),
                tcp_args.egress_args.ingress_id.clone(),
                tunnel_name.clone(),
                server_address,
                client_args.cert_subject_alt_name.clone(),
                certificate_hash.clone(),
            )
            .await
        {
            Ok((tunnel, endpoint)) => (tunnel, endpoint),
            Err(e) => {
                error!(
                    server_address = %server_address,
                    ingress_id = %&tcp_args.egress_args.ingress_id.clone(),
                    "Error while connecting to server: {e}"
                );

                return;
            }
        };

        let tcp_egress = TcpEgress::new(tunnel.clone(), tcp_args.target);

        tcp_egress.start().await.unwrap();

        tokio::select! {
            _ = tunnel.closed() => {
                warn!(
                    server_address = %server_address,
                    ingress_id = %tcp_args.egress_args.ingress_id.clone(),
                    "Server disconnected, reconnecting in 5 seconds."
                );

                sleep(Duration::from_secs(5)).await;
            }
            _ = wait_for_shutdown_signal() => {
                info!("Closing...");

                tcp_egress.stop().await.unwrap();
                tunnel.close().await;
                endpoint.wait_idle().await;

                info!("Closed");

                break;
            }

        }
    }
}
