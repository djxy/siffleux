use std::net::SocketAddr;

use base64::{Engine, engine::general_purpose};
use clap::Parser;
use siffleux::{
    AuthKey, IngressId, Server, generate_self_signed_certificate, ingress::Ingress,
    tcp_ingress::TcpIngress,
};
use tokio::signal::unix::{SignalKind, signal};
use tracing::info;

use crate::cli::{Cli, Commands, TcpIngressAgrs};

mod cli;

const SERVER_NAME: &'static str = "self-host.siffleux.dev";

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    let _ = tracing_subscriber::fmt().try_init();

    match cli.command {
        Commands::Server { ingress } => match ingress {
            cli::Ingress::Tcp(tcp_args) => start_tcp_ingress(tcp_args).await,
        },
        _ => return,
    }
}

async fn start_tcp_ingress(tcp_args: TcpIngressAgrs) {
    let (cert_der, key, cert_hash) = generate_self_signed_certificate(SERVER_NAME);

    let provided_auth_key = tcp_args.auth_key.is_some();
    let auth_key = tcp_args.auth_key.unwrap_or_else(|| {
        let mut buf = [0u8; 32];

        getrandom::fill(&mut buf).unwrap();

        AuthKey::try_from(general_purpose::URL_SAFE.encode(buf)).unwrap()
    });
    let ingress_id = tcp_args.ingress_id.unwrap_or_else(|| {
        let mut buf = [0u8; 32];

        getrandom::fill(&mut buf).unwrap();

        IngressId::try_from(general_purpose::URL_SAFE.encode(buf)).unwrap()
    });

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
        --cert-hash {cert_hash}
        ",
        if provided_auth_key {
            "<AUTH_KEY>"
        } else {
            auth_key.to_str()
        }
    );

    shutdown_signal().await;

    tcp_ingress.stop().await.unwrap();
    server.stop().await.unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    let sigterm = async {
        signal(SignalKind::terminate())
            .expect("Failed to listen SIGTERM signal.")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = sigterm => {},
    }
}
