mod cli;
mod server_tcp_ingress;
mod utils;

use clap::Parser;

use crate::{
    cli::{Cli, Commands, IngressSubCommand},
    server_tcp_ingress::start_server_tcp_ingress,
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
            IngressSubCommand::Tcp(tcp_args) => {
                start_server_tcp_ingress(server_command.server_args, tcp_args).await
            }
        },
        _ => return,
    }
}
