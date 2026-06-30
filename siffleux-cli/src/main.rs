mod cli;
mod client;
mod config;
mod server;
mod toml_config;
mod utils;

use clap::Parser;
use rustls::crypto::aws_lc_rs;
use tokio::fs::read_to_string;
use tracing::{Level, info};

use crate::{
    cli::{Cli, Commands, EgressCommand, IngressCommand},
    client::launch_client_with_egresses,
    server::launch_server_with_ingresses,
    toml_config::ServerToml,
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
        Commands::Server(server_command) => {
            if let Some(config_path) = server_command.config {
                let contents = read_to_string(config_path).await.unwrap();
                let file_config: ServerToml = toml::from_str(&contents).unwrap();
                info!("file_config={:?}", file_config);
            } else if let Some(ingress_command) = server_command.ingress {
                match ingress_command {
                    IngressCommand::Tcp(tcp_ingress_args) => {
                        launch_server_with_ingresses(
                            server_command.server_args.into(),
                            vec![tcp_ingress_args.into()],
                        )
                        .await;
                    }
                }
            }
        }
        Commands::Client(client_command) => match client_command.egress {
            EgressCommand::Tcp(tcp_egress_args) => {
                launch_client_with_egresses(vec![tcp_egress_args.into()]).await
            }
        },
    }
}
