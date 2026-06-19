mod cli;
mod client_end_egress;
mod client_tcp_egress;
mod server_tcp_ingress;
mod utils;

use clap::Parser;
use rustls::crypto::aws_lc_rs;
use tracing::Level;

use crate::{
    cli::{Cli, Commands, EgressCommand, IngressCommand},
    client_end_egress::start_end_egress,
    client_tcp_egress::start_tcp_egress,
    server_tcp_ingress::start_tcp_ingress,
};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(false)
        .with_max_level(if cli.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
        .init();

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
            EgressCommand::End(end_args) => {
                start_end_egress(client_command.client_args, end_args).await
            }
        },
    }
}
