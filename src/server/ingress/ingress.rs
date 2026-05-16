use crate::server::server_tunnel::ServerTunnel;

pub trait Ingress {
    fn on_tunnel_connected(tunnel: ServerTunnel);
}
