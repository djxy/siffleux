use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use clap::Parser;
use rand::RngExt;
use siffleux::{
    AuthKey, IngressId, Server, generate_self_signed_certificate, ingress::Ingress,
    tcp_ingress::TcpIngress,
};

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

    let auth_key = tcp_args.auth_key.unwrap_or_else(|| {
        AuthKey::try_from(hex::encode(
            rand::rng().random_iter().take(8).collect::<Vec<u8>>(),
        ))
        .unwrap()
    });
    let ingress_id = tcp_args.ingress_id.unwrap_or_else(|| {
        IngressId::try_from(hex::encode(
            rand::rng().random_iter().take(8).collect::<Vec<u8>>(),
        ))
        .unwrap()
    });

    let server =
        Server::new_with_certificate(auth_key.clone(), cert_der.clone(), key.clone_key()).unwrap();

    server
        .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .unwrap();

    let tcp_ingress = TcpIngress::new(
        ingress_id.clone(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    );

    tcp_ingress.start().await.unwrap();
}
