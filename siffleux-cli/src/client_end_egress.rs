use std::time::Duration;

use base64::Engine;
use siffleux::{Client, Egress, EndEgress, TcpEgress, TunnelName};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::{
    cli::{ClientArgs, EndEgressAgrs},
    utils::{BASE64_ENGINE, generate_secure_random_key, wait_for_shutdown_signal},
};

pub async fn start_end_egress(tunnel_args: ClientArgs, end_args: EndEgressAgrs) {
    let client = Client::new();
    let tunnel_name = tunnel_args
        .name
        .unwrap_or_else(|| TunnelName::try_from(generate_secure_random_key::<16>()).unwrap());
    let certificate_hash = BASE64_ENGINE.decode(tunnel_args.cert_hash).unwrap();

    loop {
        let (tunnel, endpoint) = match client
            .connect_tunnel_with_certificate_hash(
                end_args.egress_args.auth_key.clone(),
                end_args.egress_args.ingress_id.clone(),
                tunnel_name.clone(),
                tunnel_args.server,
                tunnel_args.cert_subject_alt_name.clone(),
                certificate_hash.clone(),
            )
            .await
        {
            Ok((tunnel, endpoint)) => (tunnel, endpoint),
            Err(e) => {
                error!(
                    server = %tunnel_args.server,
                    ingress_id = %end_args.egress_args.ingress_id.clone(),
                    "Error while connecting to server: {e}"
                );

                return;
            }
        };

        let end_egress = EndEgress::new(tunnel.clone());

        end_egress.start().await.unwrap();

        tokio::select! {
            _ = tunnel.closed() => {
                warn!(
                    server = %tunnel_args.server,
                    ingress_id = %&end_args.egress_args.ingress_id.clone(),
                    "Server disconnected, reconnecting in 5 seconds."
                );

                sleep(Duration::from_secs(5)).await;
            }
            _ = wait_for_shutdown_signal() => {
                info!("Closing...");

                end_egress.stop().await.unwrap();
                tunnel.close().await;
                endpoint.wait_idle().await;

                info!("Closed");

                break;
            }

        }
    }
}
