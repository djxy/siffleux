use std::net::{IpAddr, SocketAddr};

use siffleux::{AuthKey, IngressId, TunnelName};

// #########################
// Server Config
// #########################

pub struct ServerConfig {
    /// IP address the server will listen for tunnel connections
    pub tunnel_ip: IpAddr,

    /// Port the server will listen for tunnel connections
    pub tunnel_port: u16,

    /// Certificate subject alt name
    pub cert_subject_alt_name: String,
}

pub enum IngressConfig {
    TCP(TcpIngressConfig),
}

pub struct TcpIngressConfig {
    /// IP address the TCP ingress will listen for TCP connections
    pub ip: IpAddr,

    /// Port the TCP ingress will listen for TCP connections
    pub port: u16,

    /// ID of the ingress to connect the tunnel
    pub ingress_id: Option<IngressId>,

    /// Authentication key used to connect the tunnel to the ingress.
    pub auth_key: Option<AuthKey>,
}

// #########################
// Client Config
// #########################

pub struct TunnelConfig {
    /// Address (hostname:port or ip:port) of the server to connect the tunnel
    pub server: String,

    /// Name to identify the tunnel on the server
    pub name: Option<TunnelName>,

    /// Hash of the server certificate to validate
    pub cert_hash: String,

    /// Certificate subject alt name
    pub cert_subject_alt_name: String,
}

pub enum EgressConfig {
    TCP(TcpEgressConfig),
}

pub struct TcpEgressConfig {
    pub tunnel_config: TunnelConfig,

    /// ID of the ingress to receive ingress connections
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    pub auth_key: AuthKey,

    /// Address (ip:port) to send the TCP connections received from the ingress
    pub target: SocketAddr,
}
