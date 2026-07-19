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
- [Configuration](#configuration)

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

## Configuration

Both client and server are configured with TOML format. You need to create a separate file for client and server.

### Server Configuration

```toml
id = "my-server-id"
ip = "0.0.0.0"
port = 8765

[[tcp_ingress]]
ip = "0.0.0.0"
port = 8080
id = "ingress-1"
auth_key = "your-secret-auth-key-1"

[[tcp_ingress]]
ip = "0.0.0.0"
port = 8081
id = "ingress-2"
auth_key = "your-secret-auth-key-2"
```

At the root of the file, you configure the server.

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | No | Identifier for the server. If omitted, a random ID is generated. |
| `ip` | string | No | IP address the server listens on for client connections. Defaults to `0.0.0.0`. |
| `port` | integer | No | Port the server listens on for client connections. Defaults to `8765`. |
| `certificate_subject_alt_name` | string | No | Subject Alternative Name used for the server's TLS certificate. Defaults to `self-host.siffleux.dev`. Only required to change if you use your own certificate. |

#### `[[tcp_ingress]]`

Each entry defines a TCP listener that accepts incoming connections and tunnels them to a client egress.

| Field | Type | Required | Description |
|---|---|---|---|
| `auth_key` | string | Yes | Authentication key required to connect to this ingress. Must match the `auth_key` configured on the corresponding client egress. |
| `port` | integer | Yes | Port this ingress listens on for incoming TCP connections. |
| `ip` | string | No | IP address this ingress listens on. Defaults to `0.0.0.0`. |
| `id` | string | No | Identifier for this ingress. If omitted, a random ID is generated. Clients reference this ID via `ingress_id` to attach their egress. |

### Client Configuration

```toml
[server]
address = "example.com:8765"
certificate_hash = "sha256-hash-of-server-certificate"
certificate_subject_alt_name = "self-host.siffleux.dev"

[[tcp_egress]]
id = "egress-1"
ingress_id = "ingress-1"
auth_key = "your-secret-auth-key-1"
target = "127.0.0.1:80"

[[tcp_egress]]
id = "egress-2"
ingress_id = "ingress-2"
auth_key = "your-secret-auth-key-2"
target = "127.0.0.1:3000"
```

#### `[server]`

Defines the default server to connect to for all egresses. Can be overridden per-egress (see below).

| Field | Type | Required | Description |
|---|---|---|---|
| `address` | string | Yes | Address (`hostname:port` or `ip:port`) of the server to connect to. |
| `certificate_hash` | string | Yes | Expected hash of the server's TLS certificate, used for validation. |
| `certificate_subject_alt_name` | string | No | Expected Subject Alternative Name on the server's certificate. Defaults to `self-host.siffleux.dev`. Only required to change if you use your own certificate. |

#### `[[tcp_egress]]`

Each entry defines a local TCP egress. It forwards the connections received on the associated ingress to a `target` reachable by the client.

| Field | Type | Required | Description |
|---|---|---|---|
| `ingress_id` | string | Yes | ID of the server-side ingress this egress attaches to. Must match an `id` from a `[[tcp_ingress]]` entry on the server. |
| `auth_key` | string | Yes | Authentication key used to authenticate with the target ingress. Must match the ingress's `auth_key`. |
| `target` | string | Yes | Address (`hostname:port` or `ip:port`) of the target service to forwards the TCP connections to. |
| `server` | `[server]` | No* | Server connection details for this specific egress. *Required if no top-level `[server]` is set. Overrides the top-level `[server]` when set. |
| `id` | string | No | Identifier for this egress. If omitted, a random ID is generated. |

---

### Notes

- Any `id` field (server, ingress, egress) is optional and will be auto-generated if not provided. Explicit IDs are useful for references across restarts or in multi-egress/multi-ingress setups.
- `auth_key` values act as shared secrets between an ingress and the egress(es). Keep them private.
