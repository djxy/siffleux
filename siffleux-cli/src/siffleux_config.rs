use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use siffleux::{AuthKey, EgressId, IngressId, ServerId};

pub const DEFAULT_SERVER_IP: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
pub const DEFAULT_SERVER_PORT: u16 = 8765;
pub const DEFAULT_SERVER_TLS_SUBJECT_ALT_NAME: &'static str = "self-host.siffleux.dev";
pub const DEFAULT_SERVER_TLS_CERTIFICATE_FILE: &'static str = "siffleux-cert.pem";
pub const DEFAULT_SERVER_TLS_PRIVATE_KEY_FILE: &'static str = "siffleux-key.pem";

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

    /// TLS configs
    pub tls: TlsConfig,
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Path to the PEM certificate
    pub cert_pem_path: PathBuf,

    /// Path to the PEM private key
    pub key_pem_path: PathBuf,

    /// Certificate subject alt name
    pub cert_subject_alt_name: String,
}

#[derive(Debug)]
pub enum IngressConfig {
    TCP(TcpIngressConfig),
    UDP(UdpIngressConfig),
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

#[derive(Debug)]
pub struct UdpIngressConfig {
    /// Socket address the UDP ingress will listen for UDP datagrams
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
    pub cert_hash: String,

    /// Certificate subject alt name
    pub cert_subject_alt_name: String,
}

pub enum EgressConfig {
    TCP(TcpEgressConfig),
    UDP(UdpEgressConfig),
}

pub struct TcpEgressConfig {
    pub authentication_config: AuthenticationConfig,

    /// ID of the egress
    pub id: EgressId,

    /// ID of the ingress to receive ingress connections
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    pub auth_key: AuthKey,

    /// Address (hostname:port or ip:port) to send the TCP connections received from the ingress
    pub target: String,
}

pub struct UdpEgressConfig {
    pub authentication_config: AuthenticationConfig,

    /// ID of the egress
    pub id: EgressId,

    /// ID of the ingress to receive ingress connections
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    pub auth_key: AuthKey,

    /// Address (hostname:port or ip:port) to send the UDP datagrams received from the ingress
    pub target: String,
}
