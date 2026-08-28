use std::{
    env,
    io::IoSliceMut,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    num::ParseIntError,
    sync::{Arc, RwLock},
};

use quinn_udp::{BATCH_SIZE, RecvMeta, UdpSocketState};
use snow::{Builder, StatelessTransportState};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::{
    net::lookup_host,
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

    let (ingress_socket, ingress_socket_state) = create_super_udp_socket(9000);
    let (client_socket, client_socket_state) = create_udp_socket(9001);

    let mut client_payload = [0u8; 1500];
    let mut client_message = [0u8; 1500];
    let (client_message_len, client_origin) = client_socket.recv_from(&mut client_message).unwrap();

    responder
        .read_message(&client_message[..client_message_len], &mut client_payload)
        .unwrap();

    let client_message_len = responder
        .write_message(b"Hi, I'm server!", &mut client_message)
        .unwrap();

    client_socket
        .send_to(&client_message[..client_message_len], client_origin)
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

    let ingress_handle = tokio::task::spawn(async move {
        let mut ingress_payload = [0u8; 1500];
        let mut client_message = [0u8; 65536];
        let mut metas = [RecvMeta::default(); BATCH_SIZE];
        let mut payload = vec![0u8; 65536 * BATCH_SIZE];
        let mut iosm: Vec<IoSliceMut> = payload
            .chunks_mut(65536)
            .take(BATCH_SIZE)
            .map(IoSliceMut::new)
            .collect();
        let mut origin_set = false;

        loop {
            ingress_socket_clone
                .async_io(tokio::io::Interest::READABLE, || -> std::io::Result<()> {
                    loop {
                        match ingress_socket_state.recv(
                            (&ingress_socket_clone).into(),
                            &mut iosm,
                            &mut metas,
                        ) {
                            Ok(count) if count > 0 => {
                                for i in 0..count {
                                    let meta = &metas[i];
                                    let packet_data = &iosm[i][..meta.len];

                                    if let Ok(client_message_len) = transport_write_state
                                        .encrypt_payload(packet_data, &mut client_message)
                                    {
                                        let _ = client_socket_clone.send_to(
                                            &client_message[..client_message_len],
                                            client_origin,
                                        );
                                    }

                                    if !origin_set && origin_lock_clone.read().unwrap().is_none() {
                                        let mut lock = origin_lock_clone.write().unwrap();
                                        *lock = Some(meta.addr);
                                        origin_set = true;
                                    }
                                }
                                continue;
                            }
                            Ok(_) => {
                                // 0 packets read, yield back to Tokio reactor
                                return Err(std::io::ErrorKind::WouldBlock.into());
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // Buffer is fully drained! Return WouldBlock so Tokio registers epoll interest again
                                return Err(e);
                            }
                            Err(e) => {
                                eprintln!("UDP receive error: {}", e);
                                return Err(e);
                            }
                        }
                    }
                })
                .await
                .unwrap();
        }
        // loop {
        //     match ingress_socket_state.recv((&ingress_socket_clone).into(), &mut iosm, &mut metas) {
        //         Ok(count) => {
        //             for i in 0..count {
        //                 let meta = &metas[i];
        //                 let packet_data = &iosm[i][..meta.len];

        //                 if let Ok(client_message_len) =
        //                     transport_write_state.encrypt_payload(packet_data, &mut client_message)
        //                 {
        //                     let _ = client_socket_clone
        //                         .send_to(&client_message[..client_message_len], client_origin);
        //                 }

        //                 if !origin_set && origin_lock_clone.read().unwrap().is_none() {
        //                     let mut lock = origin_lock_clone.write().unwrap();
        //                     *lock = Some(meta.addr);
        //                     origin_set = true;
        //                 }
        //             }

        //             println!("Received {} packets in one syscall!", count);
        //         }
        //         Err(e) => {
        //             eprintln!("UDP receive error: {}", e);
        //         }
        //     }
        // }

        // while let Ok((ingress_payload_len, origin)) =
        //     ingress_socket_clone.recv_from(&mut ingress_payload)
        // {
        //     let client_message_len = transport_write_state
        //         .encrypt_payload(&ingress_payload[..ingress_payload_len], &mut client_message)
        //         .unwrap();

        //     client_socket_clone
        //         .send_to(&client_message[..client_message_len], client_origin)
        //         .unwrap();

        //     if origin_lock_clone.read().unwrap().is_none() {
        //         let mut lock = origin_lock_clone.write().unwrap();

        //         *lock = Some(origin);
        //     }
        // }
    });

    let ingress_socket_clone = ingress_socket.clone();
    let client_socket_clone = client_socket.clone();

    let client_handle = tokio::task::spawn(async move {
        let mut ingress_payload = [0u8; 1500];
        let mut client_message = [0u8; 1500];
        let mut origin: Option<SocketAddr> = None;

        while let Ok(client_message_len) = client_socket_clone.recv(&mut client_message) {
            let ingress_payload_len = transport_read_state
                .decrypt_message(&client_message[..client_message_len], &mut ingress_payload)
                .unwrap();

            if origin.is_none()
                && let Some(origin_ref) = origin_lock.read().unwrap().as_ref()
            {
                origin = Some(origin_ref.clone());
            }

            ingress_socket_clone
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

    println!("1");

    let _ = ingress_handle.abort();
    let _ = client_handle.abort();

    shutdown_udp_socket(&client_socket);

    let _ = ingress_handle.await;
    println!("2");
    let _ = client_handle.await;
    println!("3");
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

    let (server_socket, server_socket_state) = create_udp_socket(0);
    let (target_socket, target_socket_state) = create_udp_socket(0);

    server_socket.connect(server).unwrap();

    let mut server_payload = [0u8; 1500];
    let mut server_message = [0u8; 1500];
    let mut server_message_len = initiator
        .write_message(b"Hi, I'm client!", &mut server_message)
        .unwrap();

    server_socket
        .send(&server_message[..server_message_len])
        .unwrap();

    server_message_len = server_socket.recv(&mut server_message).unwrap();

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

    let server_handle = tokio::task::spawn_blocking(move || {
        let mut target_payload = [0u8; 1500];
        let mut server_message = [0u8; 1500];

        while let Ok(server_message_len) = server_socket_clone.recv(&mut server_message) {
            let target_payload_len = transport_read_state
                .decrypt_message(&server_message[..server_message_len], &mut target_payload)
                .unwrap();

            target_socket_clone
                .send_to(&target_payload[..target_payload_len], target)
                .unwrap();
        }
    });

    let server_socket_clone = server_socket.clone();
    let target_socket_clone = target_socket.clone();

    let target_handle = tokio::task::spawn_blocking(move || {
        let mut target_payload = [0u8; 1500];
        let mut server_message = [0u8; 1500];

        while let Ok(target_payload_len) = target_socket_clone.recv(&mut target_payload) {
            let server_message_len = transport_write_state
                .encrypt_payload(&target_payload[..target_payload_len], &mut server_message)
                .unwrap();

            server_socket_clone
                .send(&server_message[..server_message_len])
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

    shutdown_udp_socket(&server_socket);
    shutdown_udp_socket(&target_socket);

    println!("1");

    let _ = server_handle.await;
    println!("2");
    let _ = target_handle.await;
    println!("3");
}

fn shutdown_udp_socket(udp_socket: &UdpSocket) {
    let socket2_ref: &Socket = unsafe { &*(udp_socket as *const UdpSocket as *const Socket) };

    let _ = socket2_ref.shutdown(std::net::Shutdown::Both);
}

fn create_udp_socket(port: u16) -> (Arc<UdpSocket>, ()) {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();

    #[cfg(unix)]
    socket.set_reuse_port(true).unwrap();
    socket.set_reuse_address(true).unwrap();

    let buffer_size = 4 * 1024 * 1024; // 4mb

    socket.set_recv_buffer_size(buffer_size).unwrap();
    socket.set_send_buffer_size(buffer_size).unwrap();

    socket
        .bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port).into())
        .unwrap();

    (Arc::new(socket.into()), ())
}

fn create_super_udp_socket(port: u16) -> (Arc<tokio::net::UdpSocket>, Arc<UdpSocketState>) {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();

    #[cfg(unix)]
    socket.set_reuse_port(true).unwrap();
    socket.set_reuse_address(true).unwrap();

    let buffer_size = 4 * 1024 * 1024; // 4mb

    socket.set_recv_buffer_size(buffer_size).unwrap();
    socket.set_send_buffer_size(buffer_size).unwrap();

    socket
        .bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port).into())
        .unwrap();

    let state = UdpSocketState::new((&socket).into()).unwrap();

    (
        Arc::new(tokio::net::UdpSocket::from_std(socket.into()).unwrap()),
        Arc::new(state),
    )
}
