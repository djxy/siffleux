use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use siffleux::{AuthKey, EgressId, IngressId, ServerId};

pub const DEFAULT_SERVER_IP: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
pub const DEFAULT_SERVER_PORT: u16 = 8765;
pub const DEFAULT_SERVER_CERT_SUBJECT_ALT_NAME: &'static str = "self-host.siffleux.dev";

pub const DEFAULT_INGRESS_IP: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);

// #########################
// Server Config
// #########################

#[derive(Debug)]
pub struct ServerConfig {
    /// ID to identify the server the client is connected to
    pub id: ServerId,

    /// Socket address the server will listen for client connections
    pub client_addr: SocketAddr,

    /// Certificate subject alt name
    pub cert_subject_alt_name: String,
}

#[derive(Debug)]
pub enum IngressConfig {
    TCP(TcpIngressConfig),
}

#[derive(Debug)]
pub struct TcpIngressConfig {
    /// Socket address the TCP ingress will listen for TCP connections
    pub addr: SocketAddr,

    /// ID of the ingress
    pub id: IngressId,

    /// Authentication key used to connect to the ingress.
    pub auth_key: AuthKey,
}

// #########################
// Client Config
// #########################

pub struct AuthenticationConfig {
    /// Address (hostname:port or ip:port) of the server to connect to
    pub server: String,

    /// Hash of the server certificate to validate
    pub certificate_hash: String,

    /// Certificate subject alt name
    pub certificate_subject_alt_name: String,
}

pub enum EgressConfig {
    TCP(TcpEgressConfig),
}

pub struct TcpEgressConfig {
    pub authentication_config: AuthenticationConfig,

    /// ID of the egress
    pub id: EgressId,

    /// ID of the ingress to receive ingress connections
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    pub auth_key: AuthKey,

    /// Address (ip:port) to send the TCP connections received from the ingress
    pub target_addr: SocketAddr,
}
