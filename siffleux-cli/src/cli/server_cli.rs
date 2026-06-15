use std::net::{IpAddr, Ipv4Addr};

use clap::{Args, Subcommand};
use siffleux::{AuthKey, IngressId};

use crate::cli::CERT_SUBJECT_ALT_NAME;

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

    /// ID of the ingress.
    #[arg(long)]
    pub ingress_id: Option<IngressId>,

    /// Authentication key used to connect to the ingress.
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
