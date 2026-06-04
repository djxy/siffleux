use std::net::{IpAddr, SocketAddr};

use clap::{Args, Parser, Subcommand};
use siffleux::{AuthKey, IngressId};

const CERT_SUBJECT_ALT_NAME: &'static str = "self-host.siffleux.dev";

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
        #[command(flatten)]
        server_args: ServerArgs,

        #[command(subcommand)]
        ingress: Ingress,
    },
    /// Start a tunnel
    Tunnel {
        #[command(subcommand)]
        egress: Egress,
    },
}

// ##########################
// Server commands and args
// ##########################

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

    /// Certificate subject alt name
    #[arg(long, default_value = CERT_SUBJECT_ALT_NAME)]
    pub cert_subject_alt_name: String,
}

#[derive(Args)]
pub struct TcpIngressAgrs {
    /// IP address the TCP ingress will listen for TCP connections
    #[arg(long, default_value = "0.0.0.0")]
    pub ingress_ip: IpAddr,

    /// Port the TCP ingress will listen for TCP connections
    #[arg(long, default_value_t = 8080)]
    pub ingress_port: u16,

    /// ID of the ingress.
    #[arg(long)]
    pub ingress_id: Option<IngressId>,

    /// Authentication key used to connect to the ingress.
    #[arg(long)]
    pub auth_key: Option<AuthKey>,
}

// ##########################
// Tunnel commands and args
// ##########################

#[derive(Subcommand)]
pub enum Egress {
    /// Start a tunnel to redirect TCP connections to a target
    Tcp(TcpEgressAgrs),
}

#[derive(Args)]
pub struct TunnelArgs {
    /// IP address of the server to connect the tunnel
    #[arg(long)]
    pub server_ip: IpAddr,

    /// Port of the server to connect the tunnel
    #[arg(long, default_value_t = 8765)]
    pub server_port: u16,

    /// Certificate subject alt name
    #[arg(long, default_value = CERT_SUBJECT_ALT_NAME)]
    pub cert_subject_alt_name: String,

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
