mod cli;
mod server_tcp_ingress;
mod tunnel_tcp_egress;
mod utils;

use clap::Parser;

use crate::{
    cli::{Cli, Commands, EgressCommand, IngressCommand},
    server_tcp_ingress::start_server_tcp_ingress,
    tunnel_tcp_egress::start_tcp_egress,
};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    tracing_subscriber::fmt().try_init().unwrap();

    match cli.command {
        Commands::Server(server_command) => match server_command.ingress {
            IngressCommand::Tcp(tcp_args) => {
                start_server_tcp_ingress(server_command.server_args, tcp_args).await
            }
        },
        Commands::Tunnel(tunnel_command) => match tunnel_command.egress {
            EgressCommand::Tcp(tcp_args) => {
                start_tcp_egress(tunnel_command.tunnel_args, tcp_args).await
            }
        },
    }
}
