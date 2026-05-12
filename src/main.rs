pub mod tunnel_client;
pub mod tunnel_server;
pub mod tunnel_stream;
pub mod packets;
pub mod tunnel_server_gateway;

use crate::tunnel_client::TunnelClient;
use crate::tunnel_server::TunnelServer;
use crate::tunnel_stream::TunnelStream;
use quinn::rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use rand::RngExt;
use rkyv::{api::high::to_bytes_with_alloc, ser::allocator::Arena};
use rkyv::{deserialize, rancor::Error, Archive, Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct Test {
    id: u32,
    name: String,
    values: Option<Vec<u8>>,
}

const SERVER_NAME: &str = "localhost";
const SERVER_PORT: u16 = 5001;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    let (cert_der, key) = generate_self_signed_cert()?;

    let server_cert = cert_der.clone();

    tokio::spawn(async move {
        let server = TunnelServer::new_with_self_signed_certificate(server_cert, key).unwrap();

        start_server(server).await.unwrap();
    });

    sleep(Duration::from_millis(100)).await;

    let client = TunnelClient::new_with_self_signed_certificate(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), SERVER_PORT),
        SERVER_NAME.to_string(),
        cert_der.clone(),
    )?;

    start_client(client).await?;

    Ok(())
}

async fn start_server(server: TunnelServer) -> anyhow::Result<()> {
    let endpoint = server.listen(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        SERVER_PORT,
    ))?;

    while let Some(conn) = endpoint.accept().await {
        let connection = conn.await?;

        println!(
            "[server] accepted connection from {:?}",
            connection.remote_address()
        );

        tokio::spawn(async move {
            while let Ok((send, recv)) = connection.accept_bi().await {
                println!(
                    "[server] accepted bidirection from {:?}",
                    connection.remote_address()
                );

                tokio::spawn(async move {
                    handle_tunnel(TunnelStream::new(send, recv)).await.unwrap();
                });
            }
        });
    }

    Ok(())
}

async fn handle_tunnel(mut tunnel: TunnelStream) -> anyhow::Result<()> {
    let mut buffer = [0u8; 2048];

    loop {
        tunnel.read(&mut buffer[..8]).await?;
        let data_length = u64::from_le_bytes(buffer[..8].try_into()?) as usize;

        tunnel.read(&mut buffer[..data_length]).await?;

        // Deserialize using rkyv
        let archived = unsafe { rkyv::access_unchecked::<ArchivedTest>(&buffer[..data_length]) };
        let value = deserialize::<Test, Error>(archived).unwrap();

        println!("[server] received: {} {:?}", data_length, value);
    }
}

async fn start_client(mut client: TunnelClient) -> anyhow::Result<()> {
    client.connect().await?;

    let mut value = Test {
        id: 0,
        name: "".to_string(),
        values: None,
    };

    // Serialize with rkyv
    let mut arena = Arena::new();

    // Open a bidirectional stream
    let mut tunnel = client.create_tunnel().await?;

    loop {
        let mut rng = rand::rng();

        let num: u8 = rng.random::<u8>();

        value.id += 1;
        value.name = num.to_string();

        value.values = Some(vec![num; (num as usize / 20)]);

        let bytes = to_bytes_with_alloc::<_, Error>(&value, arena.acquire()).unwrap();

        // Send length prefix so the server knows how many bytes to read
        let len = bytes.len() as u64;

        tunnel.send(&len.to_le_bytes()).await?;

        // Send the serialized data
        tunnel.send(&bytes).await?;

        println!("[client] sent: {:?}", value);

        // Wait briefly for the server to process before dropping the connection
        sleep(Duration::from_millis(500)).await;
    }
}

fn generate_self_signed_cert()
-> anyhow::Result<(CertificateDer<'static>, PrivatePkcs8KeyDer<'static>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    let cert_der = CertificateDer::from(cert.cert);
    let key = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    Ok((cert_der, key))
}
