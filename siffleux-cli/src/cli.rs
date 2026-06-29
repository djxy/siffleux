use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use clap::{Args, Parser, Subcommand};
use siffleux::{AuthKey, EgressId, IngressId, ServerId};

use crate::{
    config::{
        AuthenticationConfig, EgressConfig, IngressConfig, ServerConfig, TcpEgressConfig,
        TcpIngressConfig,
    },
    utils::{CERT_SUBJECT_ALT_NAME, generate_secure_random_key},
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
    Client(ClientCommand),
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
pub struct ServerArgs {
    /// ID to identify the server
    pub id: Option<ServerId>,

    /// IP address the server will listen for client connections
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    pub client_ip: IpAddr,

    /// Port the server will listen for client connections
    #[arg(long, default_value_t = 8765)]
    pub client_port: u16,

    /// Certificate subject alt name
    #[arg(long, default_value = CERT_SUBJECT_ALT_NAME)]
    pub cert_subject_alt_name: String,
}

impl Into<ServerConfig> for ServerArgs {
    fn into(self) -> ServerConfig {
        let id = self
            .id
            .unwrap_or_else(|| ServerId::try_from(generate_secure_random_key::<16>()).unwrap());

        ServerConfig {
            id,
            client_addr: SocketAddr::new(self.client_ip, self.client_port),
            cert_subject_alt_name: self.cert_subject_alt_name,
        }
    }
}

#[derive(Args)]
pub struct TcpIngressAgrs {
    /// IP address the TCP ingress will listen for TCP connections
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    pub ip: IpAddr,

    /// Port the TCP ingress will listen for TCP connections
    #[arg(long, default_value_t = 3000)]
    pub port: u16,

    /// ID of the ingress
    #[arg(long)]
    pub ingress_id: Option<IngressId>,

    /// Authentication key used to connect to the ingress.
    #[arg(long)]
    pub auth_key: Option<AuthKey>,
}

impl Into<IngressConfig> for TcpIngressAgrs {
    fn into(self) -> IngressConfig {
        let auth_key = self
            .auth_key
            .unwrap_or_else(|| AuthKey::try_from(generate_secure_random_key::<32>()).unwrap());
        let ingress_id = self
            .ingress_id
            .unwrap_or_else(|| IngressId::try_from(generate_secure_random_key::<16>()).unwrap());

        IngressConfig::TCP(TcpIngressConfig {
            ip: self.ip,
            port: self.port,
            ingress_id,
            auth_key,
        })
    }
}

// #########################
// Client CLI
// #########################

#[derive(Args)]
pub struct ClientCommand {
    #[command(subcommand)]
    pub egress: EgressCommand,
}

#[derive(Subcommand)]
pub enum EgressCommand {
    /// Start a TCP egress to redirect TCP connections to a target
    Tcp(TcpEgressAgrs),
}

#[derive(Args)]
pub struct AuthenticationArgs {
    /// Address (hostname:port or ip:port) of the server to connect to
    #[arg(long)]
    pub server: String,

    /// Hash of the server certificate to validate
    #[arg(long)]
    pub cert_hash: String,

    /// Certificate subject alt name
    #[arg(long, default_value = CERT_SUBJECT_ALT_NAME)]
    pub cert_subject_alt_name: String,
}

impl Into<AuthenticationConfig> for AuthenticationArgs {
    fn into(self) -> AuthenticationConfig {
        AuthenticationConfig {
            server: self.server,
            cert_hash: self.cert_hash,
            cert_subject_alt_name: self.cert_subject_alt_name,
        }
    }
}

#[derive(Args)]
pub struct EgressAgrs {
    /// ID of the egress
    #[arg(long)]
    pub id: EgressId,

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
    pub authentication_args: AuthenticationArgs,

    #[command(flatten)]
    pub egress_args: EgressAgrs,

    /// Address (ip:port) to send the TCP connections received from the ingress
    #[arg(long)]
    pub target: SocketAddr,
}

impl Into<EgressConfig> for TcpEgressAgrs {
    fn into(self) -> EgressConfig {
        EgressConfig::TCP(TcpEgressConfig {
            authentication_config: self.authentication_args.into(),
            id: self.egress_args.id,
            ingress_id: self.egress_args.ingress_id,
            auth_key: self.egress_args.auth_key,
            target_addr: self.target,
        })
    }
}
