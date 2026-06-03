mod cli;
mod server_tcp_ingress;

use clap::Parser;
use tokio::signal::unix::{SignalKind, signal};

use crate::{
    cli::{Cli, Commands, Ingress},
    server_tcp_ingress::start_tcp_ingress,
};

const SERVER_NAME: &'static str = "self-host.siffleux.dev";

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    tracing_subscriber::fmt().try_init().unwrap();

    match cli.command {
        Commands::Server { ingress } => match ingress {
            Ingress::Tcp(tcp_args) => start_tcp_ingress(tcp_args).await,
        },
        _ => return,
    }

    shutdown_signal().await;
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
