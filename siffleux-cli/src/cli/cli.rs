use clap::{Parser, Subcommand};

use crate::cli::{ServerCommand, TunnelCommand};

pub const CERT_SUBJECT_ALT_NAME: &'static str = "self-host.siffleux.dev";

#[derive(Parser)]
#[command(name = "siffleux", version, about = "Does awesome things")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start a server
    Server(ServerCommand),
    /// Start a tunnel
    Tunnel(TunnelCommand),
}
