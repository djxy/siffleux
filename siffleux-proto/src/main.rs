use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::ParseIntError,
    sync::{Arc, RwLock},
};

use snow::{Builder, StatelessTransportState};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::{
    net::{UdpSocket, lookup_host},
    signal::unix::{SignalKind, signal},
};

const NOISE_PROTOCOL: &'static str = "Noise_NN_25519_ChaChaPoly_BLAKE2s";

pub struct WriteState {
    transport: Arc<StatelessTransportState>,
    nonce: u64,
}

pub struct ReadState {
    transport: Arc<StatelessTransportState>,
    nonce: u64,
}

impl WriteState {
    pub fn encrypt_payload(
        &mut self,
        payload: &[u8],
        message: &mut [u8],
    ) -> Result<usize, snow::Error> {
        self.nonce += 1;
        self.transport.write_message(self.nonce, payload, message)
    }
}

impl ReadState {
    pub fn decrypt_message(
        &mut self,
        message: &[u8],
        payload: &mut [u8],
    ) -> Result<usize, snow::Error> {
        self.nonce += 1;
        self.transport.read_message(self.nonce, message, payload)
    }
}

// fn encode_hex(bytes: Vec<u8>) -> String {
//     bytes
//         .iter()
//         .fold(String::with_capacity(bytes.len() * 2), |mut str, b| {
//             write!(str, "{b:02x}").unwrap();
//             str
//         })
// }

async fn lookup_address(address: &str) -> Result<SocketAddr, String> {
    lookup_host(address)
        .await
        .ok()
        .and_then(|mut a| a.next())
        .ok_or_else(|| format!("Invalid address: {}", address))
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16))
        .collect()
}

#[tokio::main]
async fn main() {
    match env::args().nth(1).as_deref() {
        Some("server") => {
            start_server(env::args().nth(2).as_deref().unwrap()).await;
        }
        Some("client") => {
            start_client(
                lookup_address(env::args().nth(2).as_deref().unwrap())
                    .await
                    .unwrap(),
                lookup_address(env::args().nth(3).as_deref().unwrap())
                    .await
                    .unwrap(),
                env::args().nth(4).as_deref().unwrap(),
            )
            .await;
        }
        Some(_) => {}
        None => {}
    }
}

async fn start_server(private_key: &str) {
    let mut responder = Builder::new(NOISE_PROTOCOL.parse().unwrap())
        .local_private_key(&decode_hex(private_key).expect("private key failed"))
        .unwrap()
        .build_responder()
        .unwrap();

    let ingress_socket = create_udp_socket(9000);
    let client_socket = create_udp_socket(9001);

    let mut client_payload = [0u8; 1500];
    let mut client_message = [0u8; 1500];
    let (client_message_len, client_origin) =
        client_socket.recv_from(&mut client_message).await.unwrap();

    responder
        .read_message(&client_message[..client_message_len], &mut client_payload)
        .unwrap();

    let client_message_len = responder
        .write_message(b"Hi, I'm server!", &mut client_message)
        .unwrap();

    client_socket
        .send_to(&client_message[..client_message_len], client_origin)
        .await
        .unwrap();

    let transport = Arc::from(responder.into_stateless_transport_mode().unwrap());
    let mut transport_write_state = WriteState {
        transport: transport.clone(),
        nonce: 0,
    };
    let mut transport_read_state = ReadState {
        transport,
        nonce: 0,
    };

    let ingress_socket_clone = ingress_socket.clone();
    let client_socket_clone = client_socket.clone();

    let origin_lock: Arc<RwLock<Option<SocketAddr>>> = Arc::new(RwLock::new(None));
    let origin_lock_clone = origin_lock.clone();

    let ingress_handle = tokio::spawn(async move {
        let mut ingress_payload = [0u8; 1500];
        let mut client_message = [0u8; 1500];

        while let Ok((ingress_payload_len, origin)) =
            ingress_socket_clone.recv_from(&mut ingress_payload).await
        {
            let client_message_len = transport_write_state
                .encrypt_payload(&ingress_payload[..ingress_payload_len], &mut client_message)
                .unwrap();

            client_socket_clone
                .send_to(&client_message[..client_message_len], client_origin)
                .await
                .unwrap();

            if origin_lock_clone.read().unwrap().is_none() {
                let mut lock = origin_lock_clone.write().unwrap();

                *lock = Some(origin);
            }
        }
    });

    let client_handle = tokio::spawn(async move {
        let mut ingress_payload = [0u8; 1500];
        let mut client_message = [0u8; 1500];
        let mut origin: Option<SocketAddr> = None;

        while let Ok(client_message_len) = client_socket.recv(&mut client_message).await {
            let ingress_payload_len = transport_read_state
                .decrypt_message(&client_message[..client_message_len], &mut ingress_payload)
                .unwrap();

            if origin.is_none()
                && let Some(origin_ref) = origin_lock.read().unwrap().as_ref()
            {
                origin = Some(origin_ref.clone());
            }

            ingress_socket
                .send_to(&ingress_payload[..ingress_payload_len], origin.unwrap())
                .await
                .unwrap();
        }
    });

    let ctrl_c = tokio::signal::ctrl_c();

    let sigterm = async {
        signal(SignalKind::terminate())
            .expect("Failed to listen SIGTERM signal.")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = sigterm => {},
    }

    client_handle.abort();
    ingress_handle.abort();

    let _ = ingress_handle.await;
    let _ = client_handle.await;
}

async fn start_client(server: SocketAddr, target: SocketAddr, server_public_key: &str) {
    let builder = Builder::new(NOISE_PROTOCOL.parse().unwrap());
    let static_initiator = builder.generate_keypair().unwrap();
    let mut initiator = builder
        .local_private_key(&static_initiator.private)
        .unwrap()
        .remote_public_key(&decode_hex(server_public_key).expect("server_public_key failed"))
        .unwrap()
        .build_initiator()
        .unwrap();

    let server_socket = create_udp_socket(0);
    let target_socket = create_udp_socket(0);

    server_socket.connect(server).await.unwrap();
    server_socket.writable().await.unwrap();

    let mut server_payload = [0u8; 1500];
    let mut server_message = [0u8; 1500];
    let mut server_message_len = initiator
        .write_message(b"Hi, I'm client!", &mut server_message)
        .unwrap();

    server_socket
        .send(&server_message[..server_message_len])
        .await
        .unwrap();

    server_message_len = server_socket.recv(&mut server_message).await.unwrap();

    initiator
        .read_message(&server_message[..server_message_len], &mut server_payload)
        .unwrap();

    let transport = Arc::from(initiator.into_stateless_transport_mode().unwrap());
    let mut transport_write_state = WriteState {
        transport: transport.clone(),
        nonce: 0,
    };
    let mut transport_read_state = ReadState {
        transport,
        nonce: 0,
    };

    let server_socket_clone = server_socket.clone();
    let target_socket_clone = target_socket.clone();

    let server_handle = tokio::spawn(async move {
        let mut target_payload = [0u8; 1500];
        let mut server_message = [0u8; 1500];

        while let Ok(server_message_len) = server_socket_clone.recv(&mut server_message).await {
            let target_payload_len = transport_read_state
                .decrypt_message(&server_message[..server_message_len], &mut target_payload)
                .unwrap();

            target_socket_clone
                .send_to(&target_payload[..target_payload_len], target)
                .await
                .unwrap();
        }
    });

    let target_handle = tokio::spawn(async move {
        let mut target_payload = [0u8; 1500];
        let mut server_message = [0u8; 1500];

        while let Ok(target_payload_len) = target_socket.recv(&mut target_payload).await {
            let server_message_len = transport_write_state
                .encrypt_payload(&target_payload[..target_payload_len], &mut server_message)
                .unwrap();

            server_socket
                .send(&server_message[..server_message_len])
                .await
                .unwrap();
        }
    });

    let ctrl_c = tokio::signal::ctrl_c();

    let sigterm = async {
        signal(SignalKind::terminate())
            .expect("Failed to listen SIGTERM signal.")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = sigterm => {},
    }

    server_handle.abort();
    target_handle.abort();

    let _ = server_handle.await;
    let _ = target_handle.await;
}

fn create_udp_socket(port: u16) -> Arc<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();

    #[cfg(unix)]
    socket.set_reuse_port(true).unwrap();
    socket.set_reuse_address(true).unwrap();
    socket.set_nonblocking(true).unwrap();

    let buffer_size = 4 * 1024 * 1024; // 4mb

    socket.set_recv_buffer_size(buffer_size).unwrap();
    socket.set_send_buffer_size(buffer_size).unwrap();

    socket
        .bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port).into())
        .unwrap();

    Arc::new(UdpSocket::from_std(socket.into()).unwrap())
}
