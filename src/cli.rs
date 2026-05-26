use std::net::{IpAddr, SocketAddr};

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
    },
    /// Start a tunnel
    Tunnel {
        #[command(subcommand)]
        egress: Egress,
    },
}

#[derive(Subcommand)]
pub enum Egress {
    /// Start a tunnel to redirect TCP connections to a target
    Tcp(TcpEgressAgrs),
}

#[derive(Subcommand)]
pub enum Ingress {
    /// Start a server with a TCP ingress
    Tcp(TcpIngressAgrs),
}

#[derive(Args)]
pub struct ServerArgs {
    /// IP address the server will listen for tunnel connections
    #[arg(long, default_value = "0.0.0.0")]
    pub tunnel_ip: IpAddr,

    /// Port the server will listen for tunnel connections
    #[arg(long, default_value_t = 8765)]
    pub tunnel_port: u16,
}

#[derive(Args)]
pub struct TcpIngressAgrs {
    /// IP address the TCP ingress will listen for TCP connections
    #[arg(long, default_value = "0.0.0.0")]
    pub ingress_ip: IpAddr,

    /// Port the TCP ingress will listen for TCP connections
    #[arg(long, default_value_t = 8080)]
    pub ingress_port: u16,

    #[command(flatten)]
    pub server_args: ServerArgs,
}

#[derive(Args)]
pub struct TunnelArgs {
    /// IP address of the server to connect the tunnel
    #[arg(long)]
    pub server_ip: IpAddr,

    /// Port of the server to connect the tunnel
    #[arg(long, default_value_t = 8765)]
    pub server_port: u16,

    /// Port of the server to connect the tunnel
    #[arg(long)]
    pub cert_hash: Option<String>,
}

#[derive(Args)]
pub struct TcpEgressAgrs {
    /// Address (ip:port) to send the TCP connections received from the server
    #[arg(long)]
    pub target: SocketAddr,

    #[command(flatten)]
    pub tunnel_args: TunnelArgs,
}
