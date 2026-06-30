use std::net::{IpAddr, SocketAddr};

use serde::Deserialize;
use siffleux::{AuthKey, EgressId, IngressId, ServerId};

// #########################
// Server Toml
// #########################

#[derive(Deserialize, Debug)]
pub struct ServerToml {
    /// ID to identify the server the client is connected to
    pub id: ServerId,

    /// Socket address the server will listen for client connections
    pub client_addr: SocketAddr,

    /// Certificate subject alt name
    pub cert_subject_alt_name: String,

    pub ingresses: Vec<IngressToml>,
}

#[derive(Deserialize, Debug)]
pub enum IngressToml {
    TCP(TcpIngressToml),
}

#[derive(Deserialize, Debug)]
pub struct TcpIngressToml {
    /// IP address the TCP ingress will listen for TCP connections
    pub ip: IpAddr,

    /// Port the TCP ingress will listen for TCP connections
    pub port: u16,

    /// ID of the ingress
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress.
    pub auth_key: AuthKey,
}

// #########################
// Client Toml
// #########################

#[derive(Deserialize, Debug)]
pub struct ClientToml {
    pub ingresses: Vec<IngressToml>,
}

#[derive(Deserialize, Debug)]
pub enum EgressToml {
    TCP(TcpEgressToml),
}

#[derive(Deserialize, Debug)]
pub struct AuthenticationToml {
    /// Address (hostname:port or ip:port) of the server to connect to
    pub server: String,

    /// Hash of the server certificate to validate
    pub cert_hash: String,

    /// Certificate subject alt name
    pub cert_subject_alt_name: String,
}

#[derive(Deserialize, Debug)]
pub struct TcpEgressToml {
    pub authentication_config: AuthenticationToml,

    /// ID of the egress
    pub id: EgressId,

    /// ID of the ingress to receive ingress connections
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    pub auth_key: AuthKey,

    /// Address (ip:port) to send the TCP connections received from the ingress
    pub target_addr: SocketAddr,
}
