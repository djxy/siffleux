use std::net::SocketAddr;

use clap::{Args, Subcommand};
use siffleux::{AuthKey, IngressId};

use crate::cli::CERT_SUBJECT_ALT_NAME;

#[derive(Args)]
pub struct TunnelSubCommand {
    #[command(flatten)]
    tunnel_args: TunnelArgs,

    #[command(subcommand)]
    egress: Egress,
}

#[derive(Args)]
pub struct TunnelArgs {
    /// Address (ip:port) of the server to connect the tunnel
    #[arg(long)]
    pub server: SocketAddr,

    /// Hash of the server certificate to validate
    #[arg(long)]
    pub cert_hash: String,

    /// Certificate subject alt name
    #[arg(long, default_value = CERT_SUBJECT_ALT_NAME)]
    pub cert_subject_alt_name: String,
}

#[derive(Subcommand)]
pub enum Egress {
    /// Start a tunnel to redirect TCP connections to a target
    Tcp(TcpEgressAgrs),
}

#[derive(Args)]
pub struct TcpEgressAgrs {
    /// ID of the ingress to receive TCP connections from
    #[arg(long)]
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    #[arg(long)]
    pub auth_key: AuthKey,

    /// Address (ip:port) to send the TCP connections received from the ingress
    #[arg(long)]
    pub target: SocketAddr,
}
