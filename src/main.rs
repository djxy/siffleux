use clap::Parser;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use sha2::{Digest, Sha256};
use siffleux::generate_self_signed_certificate;

use crate::cli::{Cli, Commands, Ingress, TcpIngressAgrs};

mod cli;

const SERVER_NAME: &'static str = "self-host.siffleux.dev";

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let _ = tracing_subscriber::fmt().try_init();

    match cli.command {
        Commands::Server { ingress } => match ingress {
            Ingress::Tcp(tcp_args) => start_tcp_ingress(tcp_args).await,
        },
        _ => return,
    }
}

async fn start_tcp_ingress(tcp_args: TcpIngressAgrs) {
    let (cert_der, key, cert_hash) = generate_self_signed_certificate(SERVER_NAME);
    println!("CERT_HASH={}", cert_hash);

    // let auth_key = AuthKey::try_from("valid_auth_key").unwrap();
    // let ingress_id = IngressId::try_from("111").unwrap();

    // let server = Server::new_with_self_signed_certificate(
    //     auth_key.clone(),
    //     cert_der.clone(),
    //     key.clone_key(),
    // )
    // .unwrap();

    // server
    //     .listen(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
    //     .await
    //     .unwrap();

    // let tcp_ingress = TcpIngress::new(
    //     ingress_id.clone(),
    //     SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    // );

    // tcp_ingress.start().await.unwrap();
}
