use std::net::{IpAddr, SocketAddr};

use serde::Deserialize;
use siffleux::{AuthKey, EgressId, IngressId, ServerId};

use crate::{
    siffleux_config::{
        AuthenticationConfig, DEFAULT_INGRESS_IP, DEFAULT_SERVER_CERT_SUBJECT_ALT_NAME,
        DEFAULT_SERVER_IP, DEFAULT_SERVER_PORT, EgressConfig, IngressConfig, ServerConfig,
        TcpEgressConfig, TcpIngressConfig,
    },
    utils::generate_secure_random_key,
};

// #########################
// Server Toml
// #########################

#[derive(Deserialize, Debug)]
pub struct ServerToml {
    /// ID to identify the server the client is connected to
    pub id: Option<ServerId>,

    /// IP address the server will listen for client connections
    pub ip: Option<IpAddr>,

    /// Port the server will listen for client connections
    pub port: Option<u16>,

    /// Certificate subject alt name
    pub certificate_subject_alt_name: Option<String>,

    pub tcp_ingress: Vec<TcpIngressToml>,
}

impl Into<(ServerConfig, Vec<IngressConfig>)> for ServerToml {
    fn into(self) -> (ServerConfig, Vec<IngressConfig>) {
        let id = self
            .id
            .unwrap_or_else(|| ServerId::try_from(generate_secure_random_key::<16>()).unwrap());
        let ip = self.ip.unwrap_or_else(|| DEFAULT_SERVER_IP);
        let port = self.port.unwrap_or_else(|| DEFAULT_SERVER_PORT);
        let cert_subject_alt_name = self
            .certificate_subject_alt_name
            .unwrap_or_else(|| DEFAULT_SERVER_CERT_SUBJECT_ALT_NAME.to_owned());

        (
            ServerConfig {
                id,
                client_addr: SocketAddr::new(ip, port),
                cert_subject_alt_name,
            },
            self.tcp_ingress
                .into_iter()
                .map(|tcp_ingress| tcp_ingress.into())
                .collect(),
        )
    }
}

#[derive(Deserialize, Debug)]
pub struct TcpIngressToml {
    /// IP address the TCP ingress will listen for TCP connections
    pub ip: Option<IpAddr>,

    /// Port the TCP ingress will listen for TCP connections
    pub port: u16,

    /// ID of the ingress
    pub id: Option<IngressId>,

    /// Authentication key used to connect to the ingress.
    pub auth_key: AuthKey,
}

impl Into<IngressConfig> for TcpIngressToml {
    fn into(self) -> IngressConfig {
        let ip = self.ip.unwrap_or_else(|| DEFAULT_INGRESS_IP);
        let id = self
            .id
            .unwrap_or_else(|| IngressId::try_from(generate_secure_random_key::<16>()).unwrap());

        IngressConfig::TCP(TcpIngressConfig {
            addr: SocketAddr::new(ip, self.port),
            id,
            auth_key: self.auth_key,
        })
    }
}

// #########################
// Client Toml
// #########################

#[derive(Deserialize, Debug)]
pub struct ClientToml {
    pub tcp_egress: Vec<TcpEgressToml>,
}

impl Into<Vec<EgressConfig>> for ClientToml {
    fn into(self) -> Vec<EgressConfig> {
        self.tcp_egress
            .into_iter()
            .map(|tcp_egress| tcp_egress.into())
            .collect()
    }
}

#[derive(Deserialize, Debug)]
pub struct AuthenticationToml {
    /// Address (hostname:port or ip:port) of the server to connect to
    pub server: String,

    /// Hash of the server certificate to validate
    pub certificate_hash: String,

    /// Certificate subject alt name
    pub certificate_subject_alt_name: Option<String>,
}

impl Into<AuthenticationConfig> for AuthenticationToml {
    fn into(self) -> AuthenticationConfig {
        let certificate_subject_alt_name = self
            .certificate_subject_alt_name
            .unwrap_or_else(|| DEFAULT_SERVER_CERT_SUBJECT_ALT_NAME.to_owned());

        AuthenticationConfig {
            server: self.server,
            certificate_hash: self.certificate_hash,
            certificate_subject_alt_name,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct TcpEgressToml {
    #[serde(flatten)]
    pub authentication_config: AuthenticationToml,

    /// ID of the egress
    pub id: Option<EgressId>,

    /// ID of the ingress to receive ingress connections
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    pub auth_key: AuthKey,

    /// Address (ip:port) to send the TCP connections received from the ingress
    pub target: SocketAddr,
}

impl Into<EgressConfig> for TcpEgressToml {
    fn into(self) -> EgressConfig {
        let id = self
            .id
            .unwrap_or_else(|| EgressId::try_from(generate_secure_random_key::<16>()).unwrap());

        EgressConfig::TCP(TcpEgressConfig {
            authentication_config: self.authentication_config.into(),
            id,
            ingress_id: self.ingress_id,
            auth_key: self.auth_key,
            target_addr: self.target,
        })
    }
}
