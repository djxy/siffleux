use std::net::SocketAddr;

use clap::{Args, Subcommand};
use siffleux::{AuthKey, IngressId, TunnelName};

use crate::cli::CERT_SUBJECT_ALT_NAME;

#[derive(Args)]
pub struct TunnelCommand {
    #[command(flatten)]
    pub tunnel_args: TunnelArgs,

    #[command(subcommand)]
    pub egress: EgressCommand,
}

#[derive(Args)]
pub struct TunnelArgs {
    /// Address (ip:port) of the server to connect the tunnel
    #[arg(long)]
    pub server: SocketAddr,

    /// ID of the ingress to receive ingress connections
    #[arg(long)]
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    #[arg(long)]
    pub auth_key: AuthKey,

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
pub struct TcpEgressAgrs {
    /// Address (ip:port) to send the TCP connections received from the ingress
    #[arg(long)]
    pub target: SocketAddr,
}
