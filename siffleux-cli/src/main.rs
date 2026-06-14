mod cli;
mod client_tcp_egress;
mod server_tcp_ingress;
mod utils;

use clap::Parser;
use tracing::Level;

use crate::{
    cli::{Cli, Commands, EgressCommand, IngressCommand},
    client_tcp_egress::start_tcp_egress,
    server_tcp_ingress::start_tcp_ingress,
};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .try_init()
        .unwrap();

    match cli.command {
        Commands::Server(server_command) => match server_command.ingress {
            IngressCommand::Tcp(tcp_args) => {
                start_tcp_ingress(server_command.server_args, tcp_args).await
            }
        },
        Commands::Client(client_command) => match client_command.egress {
            EgressCommand::Tcp(tcp_args) => {
                start_tcp_egress(client_command.client_args, tcp_args).await
            }
        },
    }
}
