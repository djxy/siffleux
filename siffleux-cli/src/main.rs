mod cli;
mod client;
mod server;
mod siffleux_config;
mod toml_config;
mod utils;

use clap::Parser;
use rustls::crypto::aws_lc_rs;
use tokio::fs::read_to_string;
use tracing::Level;

use crate::{
    cli::{Cli, Commands, EgressCommand, IngressCommand},
    client::launch_client_with_egresses,
    server::launch_server_with_ingresses,
    siffleux_config::{EgressConfig, IngressConfig, ServerConfig},
    toml_config::{ClientToml, ServerToml},
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
                let server_toml: ServerToml = toml::from_str(&contents).unwrap();
                let (server_config, ingresses): (ServerConfig, Vec<IngressConfig>) =
                    server_toml.into();

                launch_server_with_ingresses(server_config, ingresses).await;
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
        Commands::Client(client_command) => {
            if let Some(config_path) = client_command.config {
                let contents = read_to_string(config_path).await.unwrap();
                let client_toml: ClientToml = toml::from_str(&contents).unwrap();
                let egresses: Vec<EgressConfig> = client_toml.into();

                launch_client_with_egresses(egresses).await
            } else if let Some(egress_command) = client_command.egress {
                match egress_command {
                    EgressCommand::Tcp(tcp_egress_args) => {
                        launch_client_with_egresses(vec![tcp_egress_args.into()]).await
                    }
                }
            }
        }
    }
}
