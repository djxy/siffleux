use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "siffleux", version, about = "Does awesome things")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start a server
    Server {
        #[command(subcommand)]
        ingress: Ingress,

        #[command(flatten)]
        args: ServerArgs,
    },
}

#[derive(Args)]
pub struct ServerArgs {
    #[arg(long, default_value = "0.0.0.0")]
    pub tunnel_host: String,
    #[arg(long, default_value_t = 8673)]
    pub tunnel_port: u16,
}

#[derive(Subcommand)]
pub enum Ingress {
    /// Start a TCP ingress
    Tcp(TcpArgs),
}

#[derive(Args)]
pub struct TcpArgs {
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,
    #[arg(long, default_value_t = 3000)]
    pub port: u16,
}
