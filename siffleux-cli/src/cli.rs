use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand};
use siffleux::{AuthKey, EgressId, IngressId, ServerId};
use tracing::info;

use crate::{
    siffleux_config::{
        AuthenticationConfig, DEFAULT_SERVER_TLS_CERTIFICATE_FILE,
        DEFAULT_SERVER_TLS_PRIVATE_KEY_FILE, DEFAULT_SERVER_TLS_SUBJECT_ALT_NAME, EgressConfig,
        IngressConfig, ServerConfig, TcpEgressConfig, TcpIngressConfig, TlsConfig, UdpEgressConfig,
        UdpIngressConfig,
    },
    utils::generate_secure_random_key,
};

#[derive(Parser)]
#[command(name = "siffleux", version, about = "Create tunnels!")]
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
    #[arg(long)]
    pub config: Option<std::path::PathBuf>,

    #[command(flatten)]
    pub server_args: ServerArgs,

    #[command(subcommand)]
    pub ingress: Option<IngressCommand>,
}

#[derive(Subcommand)]
pub enum IngressCommand {
    /// Start a server with a TCP ingress
    Tcp(TcpIngressAgrs),

    /// Start a server with a UDP ingress
    Udp(UdpIngressAgrs),
}

#[derive(Args)]
pub struct ServerArgs {
    /// ID to identify the server
    #[arg(long)]
    pub id: Option<ServerId>,

    /// IP address the server will listen for client connections
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    pub ip: IpAddr,

    /// Port the server will listen for client connections
    #[arg(long, default_value_t = 8765)]
    pub port: u16,

    #[command(flatten)]
    pub tls: TlsArgs,
}

#[derive(Args)]
pub struct TlsArgs {
    /// Path to the PEM certificate
    #[arg(long, default_value = DEFAULT_SERVER_TLS_CERTIFICATE_FILE)]
    pub cert_pem_path: String,

    /// Path to the PEM private key
    #[arg(long, default_value = DEFAULT_SERVER_TLS_PRIVATE_KEY_FILE)]
    pub key_pem_path: String,

    /// Certificate subject alt name
    #[arg(long, default_value = DEFAULT_SERVER_TLS_SUBJECT_ALT_NAME)]
    pub cert_subject_alt_name: String,
}

impl Into<ServerConfig> for ServerArgs {
    fn into(self) -> ServerConfig {
        let id = self
            .id
            .unwrap_or_else(|| ServerId::try_from(generate_secure_random_key::<16>()).unwrap());

        ServerConfig {
            id,
            client_addr: SocketAddr::new(self.ip, self.port),
            tls: TlsConfig {
                cert_pem_path: PathBuf::from(self.tls.cert_pem_path),
                key_pem_path: PathBuf::from(self.tls.key_pem_path),
                cert_subject_alt_name: self.tls.cert_subject_alt_name,
            },
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
    pub id: Option<IngressId>,

    /// Authentication key used to connect to the ingress.
    #[arg(long)]
    pub auth_key: Option<AuthKey>,
}

impl Into<IngressConfig> for TcpIngressAgrs {
    fn into(self) -> IngressConfig {
        let auth_key = self.auth_key.unwrap_or_else(|| {
            let generate_value = generate_secure_random_key::<32>();

            info!("Generated auth key: {generate_value}");

            AuthKey::try_from(generate_value).unwrap()
        });
        let id = self
            .id
            .unwrap_or_else(|| IngressId::try_from(generate_secure_random_key::<16>()).unwrap());

        IngressConfig::TCP(TcpIngressConfig {
            addr: SocketAddr::new(self.ip, self.port),
            id,
            auth_key,
        })
    }
}

#[derive(Args)]
pub struct UdpIngressAgrs {
    /// IP address the UDP ingress will listen for UDP datagrams
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED))]
    pub ip: IpAddr,

    /// Port the UDP ingress will listen for UDP datagrams
    #[arg(long, default_value_t = 3000)]
    pub port: u16,

    /// ID of the ingress
    #[arg(long)]
    pub id: Option<IngressId>,

    /// Authentication key used to connect to the ingress.
    #[arg(long)]
    pub auth_key: Option<AuthKey>,
}

impl Into<IngressConfig> for UdpIngressAgrs {
    fn into(self) -> IngressConfig {
        let auth_key = self.auth_key.unwrap_or_else(|| {
            let generate_value = generate_secure_random_key::<32>();

            info!("Generated auth key: {generate_value}");

            AuthKey::try_from(generate_value).unwrap()
        });
        let id = self
            .id
            .unwrap_or_else(|| IngressId::try_from(generate_secure_random_key::<16>()).unwrap());

        IngressConfig::UDP(UdpIngressConfig {
            addr: SocketAddr::new(self.ip, self.port),
            id,
            auth_key,
        })
    }
}

// #########################
// Client CLI
// #########################

#[derive(Args)]
pub struct ClientCommand {
    #[arg(long)]
    pub config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub egress: Option<EgressCommand>,
}

#[derive(Subcommand)]
pub enum EgressCommand {
    /// Start a TCP egress to redirect TCP connections to a target
    Tcp(TcpEgressAgrs),

    /// Start a UDP egress to redirect UDP datagrams to a target
    Udp(UdpEgressAgrs),
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
    #[arg(long, default_value = DEFAULT_SERVER_TLS_SUBJECT_ALT_NAME)]
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
    pub id: Option<EgressId>,

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

    /// Address (hostname:port or ip:port) to send the TCP connections received from the ingress
    #[arg(long)]
    pub target: String,
}

impl Into<EgressConfig> for TcpEgressAgrs {
    fn into(self) -> EgressConfig {
        let id = self
            .egress_args
            .id
            .unwrap_or_else(|| EgressId::try_from(generate_secure_random_key::<16>()).unwrap());

        EgressConfig::TCP(TcpEgressConfig {
            authentication_config: self.authentication_args.into(),
            id,
            ingress_id: self.egress_args.ingress_id,
            auth_key: self.egress_args.auth_key,
            target: self.target,
        })
    }
}

#[derive(Args)]
pub struct UdpEgressAgrs {
    #[command(flatten)]
    pub authentication_args: AuthenticationArgs,

    #[command(flatten)]
    pub egress_args: EgressAgrs,

    /// Address (hostname:port or ip:port) to send the UDP datagrams received from the ingress
    #[arg(long)]
    pub target: String,
}

impl Into<EgressConfig> for UdpEgressAgrs {
    fn into(self) -> EgressConfig {
        let id = self
            .egress_args
            .id
            .unwrap_or_else(|| EgressId::try_from(generate_secure_random_key::<16>()).unwrap());

        EgressConfig::UDP(UdpEgressConfig {
            authentication_config: self.authentication_args.into(),
            id,
            ingress_id: self.egress_args.ingress_id,
            auth_key: self.egress_args.auth_key,
            target: self.target,
        })
    }
}
