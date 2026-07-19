<div align="center">
    <img src="./assets/logo.png" alt="Siffleux logo" width="200">

# Siffleux

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Docker Image](https://img.shields.io/badge/docker-ghcr.io-blue.svg)](https://github.com/djxy/siffleux/pkgs/container/siffleux)

> **Siffleux** (pronounced *sif-lø*) is the French-Canadian name for a groundhog.

</div>

Siffleux is a Rust-based tunneling software built with [QUIC](https://en.wikipedia.org/wiki/QUIC). Expose services hosted behind a NAT to the internet without opening ingress ports.

- [Features](#features)
- [Installation](#installation)
- [Quickstart](#quickstart)
- [How it works](#how-it-works)

## Features

- **No Open Ports**: The client connects to the server, so nothing needs to be configured on your NAT or firewall.
- **High Performance**: Handles 10,000+ concurrent connections and multi-gigabit throughput per second.
- **Security**: Traffic is encrypted end-to-end using QUIC's built-in TLS 1.3
- **Multi-platform**: Binaries for Linux and macOS  and Docker images.
- **Load Balancing**: Connect multiple egresses per ingress endpoint to automatically distribute traffic across multiple instances.

## Installation

You can install Siffleux using one of 3 methods.

### Binary
Download the binary from the [latest release](https://github.com/djxy/siffleux/releases).

### Docker
Pull the Docker image.

```bash
docker pull ghcr.io/djxy/siffleux:latest
```

### Build the source
Ensure you have the Rust toolchain installed, then clone and compile:

```bash
git clone https://github.com/djxy/siffleux
cd siffleux
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

## Quickstart

Follow this workflow to test a local tunnel setup using an Nginx backend.

### 1. Start the server

Run the Siffleux server. It opens a TCP ingress on port `3000` and listens for incoming tunnel clients on port `8765`.

```bash
siffleux server tcp
```

The server logs output the certificate hash, ingress ID and the auth key. You will need these values to configure the client.

```
Generated auth key: $AUTH_KEY
Loaded self signed certificate
Certificate hash: $CERTIFICATE_HASH
Starting listening for tunnels...
Ready to accept tunnels.
Starting TCP ingress... ingress_id=$INGRESS_ID
Ready to accept TCP connections on 0.0.0.0:3000. ingress_id=$INGRESS_ID
```

### 2. Start the client

Run the Siffleux client to establish a tunnel by passing the values from the server logs.

```bash
siffleux client tcp \
  --server 127.0.0.1:8765 \
  --certificate-hash $CERTIFICATE_HASH \
  --ingress-id $INGRESS_ID \
  --auth-key $AUTH_KEY \
  --target 127.0.0.1:80
```

### 3. Start the target service 

Spin up a web server with Nginx to receive the tunneled traffic.

```bash
docker run -p 80:80 nginx
```

### 4. Test the tunnel

You can now access your Nginx instance through the TCP ingress.

```bash
curl http://localhost:3000
```

## How it works

Siffleux works by establishing a persistent QUIC connection between a private client, running inside your private network, and a public server, running on a reachable host or the internet. The client initiates the connection to the server to traverse NATs. Once connected, the server forwards all incoming connections to the client through the tunnel. The client forwards them to the services in your private network.

```
Egress
Endpoints
┌────────────┐
│Database    │◄──┐                   ┌────┐
├────────────┤   │            ┌──────│QUIC│──────┐
│Media Server│◄──┤            │      └────┘      │
├────────────┤   │  ┌──────┐  │  ┌────────────┐  │  ┌──────┐     Ingress
│Storage     │◄──┼──┼Client┼──┴─►│NAT/Firewall├──┴─►│Server│◄────Endpoints
├────────────┤   │  └──────┘     └────────────┘     └──────┘
│Game Server │◄──┤
├────────────┤   │
│Web Server  │◄──┘
└────────────┘
```

When a connection hits an ingress endpoint on the server, the data is tunneled through a QUIC stream to the client. The client will proxy the data received to your target services. If multiple egresses bind to the same ingress, the server automatically round-robins traffic across them.
