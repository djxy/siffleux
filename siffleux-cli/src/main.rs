mod cli;
mod client;
mod config;
mod server;
mod utils;

use clap::Parser;
use rustls::crypto::aws_lc_rs;
use tracing::Level;

use crate::{
    cli::{Cli, Commands, EgressCommand, IngressCommand},
    server::launch_server_with_ingresses,
};

#[tokio::main]
async fn main() {
    // let content = std::fs::read_to_string("config.toml").expect("Could not read config.toml");

    // let cfg: Config = toml::from_str(&content).expect("Failed to parse config.toml");

    // println!("{:?}", cfg);

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
            IngressCommand::Tcp(tcp_ingress_args) => {
                launch_server_with_ingresses(
                    server_command.server_args.into(),
                    vec![tcp_ingress_args.into()],
                )
                .await;
            }
        },
        Commands::Client(client_command) => match client_command.egress {
            EgressCommand::Tcp(tcp_args) => {
                start_tcp_egress(client_command.client_args, tcp_args).await
            }
        },
    }
}
