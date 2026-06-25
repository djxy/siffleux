use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use clap::{Args, Parser, Subcommand};
use siffleux::{AuthKey, IngressId, TunnelName};

use crate::{
    config::{IngressConfig, ServerConfig, TcpIngressConfig},
    utils::CERT_SUBJECT_ALT_NAME,
};

#[derive(Parser)]
#[command(name = "siffleux", version, about = "Does awesome things")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long, default_value_t = false)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start a server
    Server(ServerCommand),
    /// Start a client
    Client(TunnelCommand),
}

// #########################
// Server CLI
// #########################

#[derive(Args)]
pub struct ServerCommand {
    #[command(flatten)]
    pub server_args: ServerArgs,

    #[command(subcommand)]
    pub ingress: IngressCommand,
}

#[derive(Subcommand)]
pub enum IngressCommand {
    /// Start a server with a TCP ingress
    Tcp(TcpIngressAgrs),
}

#[derive(Args)]
pub struct TcpIngressAgrs {
    /// IP address the TCP ingress will listen for TCP connections
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    pub ip: IpAddr,

    /// Port the TCP ingress will listen for TCP connections
    #[arg(long, default_value_t = 3000)]
    pub port: u16,

    /// ID of the ingress to connect the tunnel
    #[arg(long)]
    pub ingress_id: Option<IngressId>,

    /// Authentication key used to connect the tunnel to the ingress.
    #[arg(long)]
    pub auth_key: Option<AuthKey>,
}

#[derive(Args)]
pub struct ServerArgs {
    /// IP address the server will listen for tunnel connections
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    pub tunnel_ip: IpAddr,

    /// Port the server will listen for tunnel connections
    #[arg(long, default_value_t = 8765)]
    pub tunnel_port: u16,

    /// Certificate subject alt name
    #[arg(long, default_value = CERT_SUBJECT_ALT_NAME)]
    pub cert_subject_alt_name: String,
}

impl Into<IngressConfig> for TcpIngressAgrs {
    fn into(self) -> IngressConfig {
        IngressConfig::TCP(TcpIngressConfig {
            ip: self.ip,
            port: self.port,
            ingress_id: self.ingress_id,
            auth_key: self.auth_key,
        })
    }
}

impl Into<ServerConfig> for ServerArgs {
    fn into(self) -> ServerConfig {
        ServerConfig {
            tunnel_ip: self.tunnel_ip,
            tunnel_port: self.tunnel_port,
            cert_subject_alt_name: self.cert_subject_alt_name,
        }
    }
}

// #########################
// Client CLI
// #########################

#[derive(Args)]
pub struct TunnelCommand {
    #[command(flatten)]
    pub client_args: ClientArgs,

    #[command(subcommand)]
    pub egress: EgressCommand,
}

#[derive(Args)]
pub struct ClientArgs {
    /// Address (hostname:port or ip:port) of the server to connect the tunnel
    #[arg(long)]
    pub server: String,

    /// Name to identify the tunnel on the server
    #[arg(long)]
    pub name: Option<TunnelName>,

    /// Hash of the server certificate to validate
    #[arg(long)]
    pub cert_hash: String,

    /// Certificate subject alt name
    #[arg(long, default_value = CERT_SUBJECT_ALT_NAME)]
    pub cert_subject_alt_name: String,
}

#[derive(Subcommand)]
pub enum EgressCommand {
    /// Start a tunnel to redirect TCP connections to a target
    Tcp(TcpEgressAgrs),
}

#[derive(Args)]
pub struct EgressAgrs {
    /// ID of the ingress to receive ingress connections
    #[arg(long)]
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    #[arg(long)]
    pub auth_key: AuthKey,
}

#[derive(Args)]
pub struct TcpEgressAgrs {
    #[command(flatten)]
    pub egress_args: EgressAgrs,

    /// Address (ip:port) to send the TCP connections received from the ingress
    #[arg(long)]
    pub target: SocketAddr,
}
