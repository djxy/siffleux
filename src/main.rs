use clap::Parser;

use crate::cli::{Cli, Commands, Ingress};

mod cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Server { args, ingress } => match ingress {
            Ingress::Tcp(tcp) => {
                println!(
                    "tunnel_host={} tunnel_port={} port={} host={}",
                    args.tunnel_host, args.tunnel_port, tcp.port, tcp.host
                );
            }
        },
    }
}
