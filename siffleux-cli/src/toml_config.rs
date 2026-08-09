use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use serde::Deserialize;
use siffleux::{AuthKey, EgressId, IngressId, ServerId};

use crate::{
    siffleux_config::{
        AuthenticationConfig, DEFAULT_INGRESS_IP, DEFAULT_SERVER_IP, DEFAULT_SERVER_PORT,
        DEFAULT_SERVER_TLS_CERTIFICATE_FILE, DEFAULT_SERVER_TLS_PRIVATE_KEY_FILE,
        DEFAULT_SERVER_TLS_SUBJECT_ALT_NAME, EgressConfig, IngressConfig, ServerConfig,
        TcpEgressConfig, TcpIngressConfig, TlsConfig, UdpEgressConfig, UdpIngressConfig,
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

    /// TLS configs
    pub tls: Option<TlsToml>,

    #[serde(default)]
    pub tcp_ingress: Vec<TcpIngressToml>,

    #[serde(default)]
    pub udp_ingress: Vec<UdpIngressToml>,
}

impl Into<(ServerConfig, Vec<IngressConfig>)> for ServerToml {
    fn into(self) -> (ServerConfig, Vec<IngressConfig>) {
        let id = self
            .id
            .unwrap_or_else(|| ServerId::try_from(generate_secure_random_key::<16>()).unwrap());
        let ip = self.ip.unwrap_or_else(|| DEFAULT_SERVER_IP);
        let port = self.port.unwrap_or_else(|| DEFAULT_SERVER_PORT);
        let mut ingress_configs: Vec<IngressConfig> =
            Vec::with_capacity(self.tcp_ingress.len() + self.udp_ingress.len());

        ingress_configs.append(
            &mut self
                .tcp_ingress
                .into_iter()
                .map(|tcp_ingress| tcp_ingress.into())
                .collect(),
        );

        ingress_configs.append(
            &mut self
                .udp_ingress
                .into_iter()
                .map(|udp_ingress| udp_ingress.into())
                .collect(),
        );

        (
            ServerConfig {
                id,
                client_addr: SocketAddr::new(ip, port),
                tls: self.tls.map_or_else(
                    || TlsConfig {
                        cert_pem_path: PathBuf::from(DEFAULT_SERVER_TLS_CERTIFICATE_FILE),
                        key_pem_path: PathBuf::from(DEFAULT_SERVER_TLS_PRIVATE_KEY_FILE),
                        cert_subject_alt_name: DEFAULT_SERVER_TLS_SUBJECT_ALT_NAME.to_owned(),
                    },
                    |tls| TlsConfig {
                        cert_pem_path: tls.cert_pem_path,
                        key_pem_path: tls.key_pem_path,
                        cert_subject_alt_name: tls
                            .cert_subject_alt_name
                            .unwrap_or_else(|| DEFAULT_SERVER_TLS_SUBJECT_ALT_NAME.to_owned()),
                    },
                ),
            },
            ingress_configs,
        )
    }
}

#[derive(Deserialize, Debug)]
pub struct TlsToml {
    /// Path to the PEM certificate
    pub cert_pem_path: PathBuf,

    /// Path to the PEM private key
    pub key_pem_path: PathBuf,

    /// Certificate subject alt name
    pub cert_subject_alt_name: Option<String>,
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

#[derive(Deserialize, Debug)]
pub struct UdpIngressToml {
    /// IP address the UDP ingress will listen for UDP datagrams
    pub ip: Option<IpAddr>,

    /// Port the UDP ingress will listen for UDP datagrams
    pub port: u16,

    /// ID of the ingress
    pub id: Option<IngressId>,

    /// Authentication key used to connect to the ingress.
    pub auth_key: AuthKey,
}

impl Into<IngressConfig> for UdpIngressToml {
    fn into(self) -> IngressConfig {
        let ip = self.ip.unwrap_or_else(|| DEFAULT_INGRESS_IP);
        let id = self
            .id
            .unwrap_or_else(|| IngressId::try_from(generate_secure_random_key::<16>()).unwrap());

        IngressConfig::UDP(UdpIngressConfig {
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
    pub server: Option<AuthenticationToml>,

    #[serde(default)]
    pub tcp_egress: Vec<TcpEgressToml>,

    #[serde(default)]
    pub udp_egress: Vec<UdpEgressToml>,
}

impl TryFrom<ClientToml> for Vec<EgressConfig> {
    type Error = String;

    fn try_from(client_toml: ClientToml) -> Result<Self, String> {
        let mut egress_configs: Vec<EgressConfig> =
            Vec::with_capacity(client_toml.tcp_egress.len() + client_toml.udp_egress.len());
        let tcp_egress_configs: Result<Vec<EgressConfig>, String> = client_toml
            .tcp_egress
            .into_iter()
            .map(|tcp_egress| {
                let id = tcp_egress.id.unwrap_or_else(|| {
                    EgressId::try_from(generate_secure_random_key::<16>()).unwrap()
                });
                let authentication_config: AuthenticationConfig = tcp_egress
                    .server
                    .or_else(|| client_toml.server.clone())
                    .ok_or_else(|| {
                        format!(
                            "TCP egress ingress_id={} doesn't a server to connect to.",
                            tcp_egress.ingress_id
                        )
                    })?
                    .into();

                Ok(EgressConfig::TCP(TcpEgressConfig {
                    authentication_config,
                    id,
                    ingress_id: tcp_egress.ingress_id,
                    auth_key: tcp_egress.auth_key,
                    target: tcp_egress.target,
                }))
            })
            .collect();

        egress_configs.append(&mut tcp_egress_configs?);

        let udp_egress_configs: Result<Vec<EgressConfig>, String> = client_toml
            .udp_egress
            .into_iter()
            .map(|udp_egress| {
                let id = udp_egress.id.unwrap_or_else(|| {
                    EgressId::try_from(generate_secure_random_key::<16>()).unwrap()
                });
                let authentication_config: AuthenticationConfig = udp_egress
                    .server
                    .or_else(|| client_toml.server.clone())
                    .ok_or_else(|| {
                        format!(
                            "UDP egress ingress_id={} doesn't a server to connect to.",
                            udp_egress.ingress_id
                        )
                    })?
                    .into();

                Ok(EgressConfig::UDP(UdpEgressConfig {
                    authentication_config,
                    id,
                    ingress_id: udp_egress.ingress_id,
                    auth_key: udp_egress.auth_key,
                    target: udp_egress.target,
                }))
            })
            .collect();

        egress_configs.append(&mut udp_egress_configs?);

        Ok(egress_configs)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct AuthenticationToml {
    /// Address (hostname:port or ip:port) of the server to connect to
    pub address: String,

    /// Hash of the server certificate to validate
    pub cert_hash: String,

    /// Certificate subject alt name
    pub cert_subject_alt_name: Option<String>,
}

impl Into<AuthenticationConfig> for AuthenticationToml {
    fn into(self) -> AuthenticationConfig {
        let cert_subject_alt_name = self
            .cert_subject_alt_name
            .unwrap_or_else(|| DEFAULT_SERVER_TLS_SUBJECT_ALT_NAME.to_owned());

        AuthenticationConfig {
            server: self.address,
            cert_hash: self.cert_hash,
            cert_subject_alt_name,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct TcpEgressToml {
    pub server: Option<AuthenticationToml>,

    /// ID of the egress
    pub id: Option<EgressId>,

    /// ID of the ingress to receive ingress connections
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    pub auth_key: AuthKey,

    /// Address (hostname:port or ip:port) to send the TCP connections received from the ingress
    pub target: String,
}

#[derive(Deserialize, Debug)]
pub struct UdpEgressToml {
    pub server: Option<AuthenticationToml>,

    /// ID of the egress
    pub id: Option<EgressId>,

    /// ID of the ingress to receive ingress connections
    pub ingress_id: IngressId,

    /// Authentication key used to connect to the ingress
    pub auth_key: AuthKey,

    /// Address (hostname:port or ip:port) to send the UDP datagrams received from the ingress
    pub target: String,
}
