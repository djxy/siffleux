use std::net::SocketAddr;

use base64::Engine;
use siffleux::{Client, Egress, TcpEgress, authentication::V1CertifcateHash};
use tokio::net::lookup_host;

use crate::{
    siffleux_config::{
        AuthenticationConfig,
        EgressConfig::{self, TCP},
        TcpEgressConfig,
    },
    utils::{BASE64_ENGINE, wait_for_shutdown_signal},
};

pub async fn launch_client_with_egresses(egress_configs: Vec<EgressConfig>) {
    let client = Client::new();

    for egress_config in egress_configs {
        match egress_config {
            TCP(tcp_egress_config) => launch_tcp_egress(&client, &tcp_egress_config)
                .await
                .unwrap(),
        }
    }

    wait_for_shutdown_signal().await;

    client.stop().await.unwrap();
}

async fn launch_tcp_egress(
    client: &Client,
    tcp_egress_config: &TcpEgressConfig,
) -> Result<(), String> {
    let (server_address, certificate_hash) =
        prepare_server_config(&tcp_egress_config.authentication_config).await?;

    let authentication = V1CertifcateHash::new(
        client.clone(),
        tcp_egress_config.auth_key.clone(),
        server_address,
        tcp_egress_config
            .authentication_config
            .certificate_subject_alt_name
            .clone(),
        certificate_hash,
    );

    let tcp_egress = TcpEgress::new(
        tcp_egress_config.id.clone(),
        Box::new(authentication),
        tcp_egress_config.ingress_id.clone(),
        tcp_egress_config.target_addr,
    );

    tcp_egress.start().await.unwrap();

    client.assign_egress(Box::new(tcp_egress)).unwrap();

    Ok(())
}

async fn prepare_server_config(
    authentication_config: &AuthenticationConfig,
) -> Result<(SocketAddr, Vec<u8>), String> {
    let Some(server_address) = lookup_host(&authentication_config.server)
        .await
        .map_or_else(|_| None, |mut a| a.next().or(None))
    else {
        return Err(format!(
            "Invalid server address: {}",
            &authentication_config.server
        ));
    };

    let certificate_hash = BASE64_ENGINE
        .decode(authentication_config.certificate_hash.clone())
        .unwrap();

    Ok((server_address, certificate_hash))
}
