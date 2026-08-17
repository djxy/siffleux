# Siffleux

[![License: MIT](https://img.shields.io/github/license/djxy/siffleux)](https://opensource.org/licenses/MIT)
[![Docker Image](https://img.shields.io/badge/docker-ghcr.io-blue.svg)](https://github.com/djxy/siffleux/pkgs/container/siffleux)

> **Siffleux** (pronounced _sif-lø_) is the French-Canadian name for a groundhog.

Siffleux is a lightweight and self-hosted tunnel that exposes services hosted behind a NAT to the internet without opening ingress ports.

- [Features](#features)
  - [Protocols](#protocols)
  - [IP v4/v6](#ip-v4v6)
- [Installation](#installation)
- [Quickstart](#quickstart)
- [How it works](#how-it-works)
  - [Security](#security)
  - [TCP Ingress/Egress](#tcp-ingressegress)
  - [UDP Ingress/Egress](#udp-ingressegress)
  - [Load Balancing](#load-balancing)
  - [Reconnection](#reconnection)
- [Configuration](#configuration)
- [CLI](#cli)

## Features

- **No Open Ports**: The client connects to the server, so nothing needs to be configured on your NAT or firewall.
- **High Performance**: Handles 10,000+ concurrent connections and multi-gigabit throughput per second.
- **Security**: Traffic is encrypted end-to-end using [QUIC](https://en.wikipedia.org/wiki/QUIC) built-in TLS 1.3
- **Multi-platform**: Binaries for Linux and macOS and Docker images.
- **Load Balancing**: Connect multiple egresses per ingress endpoint to automatically distribute traffic across multiple instances.

### Protocols

- **TCP**: TCP ingress and egress are supported.
- **UDP**: UDP ingress and egress are supported.

**Note**: I'm planning to add layer 7 protocols(HTTP, SSH, etc...).

### IP v4/v6

Currently Siffleux **only supports IPv4**. I only tested on IPv4 locally and my ISP doesn't provide an IPv6 address. It is on the roadmap to be fixed.

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

Follow this workflow to test a local TCP tunnel setup using an Nginx backend.

### 1. Start the server

Run the Siffleux server. It opens a TCP ingress on port `3000` and listens for incoming tunnel clients on port `8765`.

```bash
siffleux server tcp
```

The server logs output the certificate hash, ingress ID and the auth key. You will need these values to configure the client.

```
Generated auth key: $AUTH_KEY
Loaded self signed certificate
Certificate hash: $CERT_HASH
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
  --cert-hash $CERT_HASH \
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

### Security

Currently Siffleux establishes TLS between the server and client with a self signed certificate and certificate pinning. The server generates the certificate on first launch and logs the certificate hash. The client uses the certificate hash to verify the server identity.

Support for certificates issued by a certificate authority is on the roadmap. I did self signed certificate first, since it is an easier solution for self hosted setup.

### TCP Ingress/Egress

When a TCP connection hits an ingress endpoint on the server, the server opens a new QUIC stream to tunnel the connection to the client. When the client receives a new QUIC stream, it will open a TCP connection to the targeted service.

### UDP Ingress/Egress

When a UDP datagram hits an ingress endpoint on the server, the server tunnels the datagram to the client using the QUIC unreliable datagrams extension([RFC 9221](https://datatracker.ietf.org/doc/html/rfc9221)). When the client receives a datagram, it will send it to the targeted service.

**Note:** As with UDP, delivery and ordering of datagrams are not guaranteed.

### Load Balancing

You can create a load balancer by assigning multiple egresses to the same ingress. The ingress will tunnel the connections to the different egresses in a round-robin way.

If an egress disconnects, the ingress will stop tunneling connections to it. The egress can reconnect to the ingress at any time to restart receiving connections.

### Reconnection

When the QUIC connection ends unexpectedly between the client and the server, the server terminates the TCP connections tunneled to the client. The connections are not kept alive while waiting for the client to reconnect. At the same time, the client tries to reconnect to the server. The wait time between retries increases exponentially, up to a maximum of 30 seconds.

## Configuration

Both client and server are configured with TOML format. You need to create a separate file for client and server.

### Server Configuration

```toml
id = "my-server-id"
ip = "0.0.0.0"
port = 8765

[tls]
cert_pem_path = "server-cert.pem"
key_pem_path = "server-key.pem"
subject_alt_name = "my-server.com"

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

[[udp_ingress]]
ip = "0.0.0.0"
port = 8082
id = "ingress-3"
auth_key = "your-secret-auth-key-3"
```

At the root of the file, you configure the server.

| Field                          | Type    | Required | Description                                                                                                                                                    |
| ------------------------------ | ------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`                           | string  | No       | Identifier for the server. If omitted, a random ID is generated.                                                                                               |
| `ip`                           | string  | No       | IP address the server listens on for client connections. Defaults to `0.0.0.0`.                                                                                |
| `port`                         | integer | No       | Port the server listens on for client connections. Defaults to `8765`. |

#### `[tls]`

Defines the TLS configuration used by the server.

**Note:** If the certificate doesn't exist on first launch. The server will generate one and save it at the specified paths.

| Field                          | Type    | Required | Description                                                                                                                                                    |
| ------------------------------ | ------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cert_pem_path` | string  | No       | Path of the PEM certificate used for TLS. Defaults to `siffleux-cert.pem`. |
| `key_pem_path` | string  | No       | Path of the private key used for TLS. Defaults to `siffleux-key.pem`. |
| `cert_subject_alt_name` | string  | No       | Subject Alternative Name of the certificate used the TLS. Defaults to `self-host.siffleux.dev`. |

#### `[[tcp_ingress]]`

Each entry defines a TCP listener that accepts incoming connections and tunnels them to a client egress.

| Field      | Type    | Required | Description                                                                                                                           |
| ---------- | ------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `auth_key` | string  | Yes      | Authentication key required to connect to this ingress. Must match the `auth_key` configured on the corresponding client egress.      |
| `port`     | integer | Yes      | Port this ingress listens on for incoming TCP connections.                                                                            |
| `ip`       | string  | No       | IP address this ingress listens on. Defaults to `0.0.0.0`.                                                                            |
| `id`       | string  | No       | Identifier for this ingress. If omitted, a random ID is generated. Clients reference this ID via `ingress_id` to attach their egress. |

#### `[[udp_ingress]]`

Each entry defines a UDP listener that accepts incoming datagrams and tunnels them to a client egress.

| Field      | Type    | Required | Description                                                                                                                           |
| ---------- | ------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `auth_key` | string  | Yes      | Authentication key required to connect to this ingress. Must match the `auth_key` configured on the corresponding client egress.      |
| `port`     | integer | Yes      | Port this ingress listens on for incoming UDP datagrams.                                                                              |
| `ip`       | string  | No       | IP address this ingress listens on. Defaults to `0.0.0.0`.                                                                            |
| `id`       | string  | No       | Identifier for this ingress. If omitted, a random ID is generated. Clients reference this ID via `ingress_id` to attach their egress. |

### Client Configuration

```toml
[server]
address = "example.com:8765"
cert_hash = "sha256-hash-of-server-certificate"
cert_subject_alt_name = "self-host.siffleux.dev"

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

[[udp_egress]]
id = "egress-3"
ingress_id = "ingress-3"
auth_key = "your-secret-auth-key-3"
target = "127.0.0.1:5000"
```

#### `[server]`

Defines the default server to connect to for all egresses. Can be overridden per-egress (see below).

| Field                          | Type   | Required | Description                                                                                                                                                   |
| ------------------------------ | ------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `address`                      | string | Yes      | Address (`hostname:port` or `ip:port`) of the server to connect to.                                                                                           |
| `cert_hash`             | string | Yes      | Expected hash of the server's TLS certificate, used for validation.                                                                                           |
| `cert_subject_alt_name` | string | No       | Expected Subject Alternative Name on the server's certificate. Defaults to `self-host.siffleux.dev`. Only required to change if you use your own certificate. |

#### `[[tcp_egress]]`

Each entry defines a local TCP egress. It forwards the connections received on the associated ingress to a `target` reachable by the client.

| Field        | Type       | Required | Description                                                                                                                                   |
| ------------ | ---------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `ingress_id` | string     | Yes      | ID of the server-side ingress this egress attaches to. Must match an `id` from a `[[tcp_ingress]]` entry on the server.                       |
| `auth_key`   | string     | Yes      | Authentication key used to authenticate with the target ingress. Must match the ingress's `auth_key`.                                         |
| `target`     | string     | Yes      | Address (`hostname:port` or `ip:port`) of the target service to forwards the TCP connections to.                                              |
| `server`     | `[server]` | No*      | Server connection details for this specific egress. *Required if no top-level `[server]` is set. Overrides the top-level `[server]` when set. |
| `id`         | string     | No       | Identifier for this egress. If omitted, a random ID is generated.                                                                             |

#### `[[udp_egress]]`

Each entry defines a local UDP egress. It forwards the datagrams received on the associated ingress to a `target` reachable by the client.

| Field        | Type       | Required | Description                                                                                                                                   |
| ------------ | ---------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `ingress_id` | string     | Yes      | ID of the server-side ingress this egress attaches to. Must match an `id` from a `[[udp_ingress]]` entry on the server.                       |
| `auth_key`   | string     | Yes      | Authentication key used to authenticate with the target ingress. Must match the ingress's `auth_key`.                                         |
| `target`     | string     | Yes      | Address (`hostname:port` or `ip:port`) of the target service to forward the UDP datagrams to.                                                 |
| `server`     | `[server]` | No*      | Server connection details for this specific egress. *Required if no top-level `[server]` is set. Overrides the top-level `[server]` when set. |
| `id`         | string     | No       | Identifier for this egress. If omitted, a random ID is generated.                                                                             |

---

### Notes

- Any `id` field (server, ingress, egress) is optional and will be auto-generated if not provided. Explicit IDs are useful for references across restarts or in multi-egress/multi-ingress setups.
- `auth_key` values act as shared secrets between an ingress and the egress(es). Keep them private.

## CLI

Siffleux can be configured directly in the CLI. It is useful to launch a 1 ingress/egress setup or to test quickly. To configure multiple ingresses/egresses, you have to use the [TOML configurations](#configuration).

### Server CLI

```bash
siffleux server [SERVER_OPTIONS] [INGRESS]
```

#### Server Options

| Argument/Option                  | Type    | Required | Description                                                                                           |
| -------------------------------- | ------- | -------- | ----------------------------------------------------------------------------------------------------- |
| `--config`                       | path    | No       | Path to a TOML server configuration file. If configured, all the other arguments won't be considered. |
| `--id`                           | string  | No       | Identifier for the server. If omitted, a random ID is generated.                                      |
| `--ip`                           | string  | No       | IP address the server listens on for client connections. Defaults to `0.0.0.0`.                       |
| `--port`                         | integer | No       | Port the server listens on for client connections. Defaults to `8765`.                                |
| `--cert-pem-path` | string  | No       | Path of the PEM certificate used for TLS. Defaults to `siffleux-cert.pem`. |
| `--key-pem-path` | string  | No       | Path of the private key used for TLS. Defaults to `siffleux-key.pem`. |
| `--cert-subject-alt-name` | string  | No       | Subject Alternative Name of the certificate used for TLS. Defaults to `self-host.siffleux.dev`. |

#### TCP Ingress

```bash
siffleux server [SERVER_OPTIONS] tcp [TCP_INGRESS_OPTIONS]
```

| Option       | Type    | Required | Description                                                                                              |
| ------------ | ------- | -------- | -------------------------------------------------------------------------------------------------------- |
| `--ip`       | string  | No       | IP address the TCP ingress listens on. Defaults to `0.0.0.0`.                                            |
| `--port`     | integer | No       | Port the TCP ingress listens on for incoming TCP connections. Defaults to `3000`.                        |
| `--id`       | string  | No       | Identifier for the ingress. If omitted, a random ID is generated.                                        |
| `--auth-key` | string  | No       | Authentication key required to connect to the ingress. If omitted, a random key is generated and logged. |

#### UDP Ingress

```bash
siffleux server [SERVER_OPTIONS] udp [UDP_INGRESS_OPTIONS]
```

| Option       | Type    | Required | Description                                                                                              |
| ------------ | ------- | -------- | -------------------------------------------------------------------------------------------------------- |
| `--ip`       | string  | No       | IP address the UDP ingress listens on. Defaults to `0.0.0.0`.                                            |
| `--port`     | integer | No       | Port the UDP ingress listens on for incoming UDP datagrams. Defaults to `3000`.                          |
| `--id`       | string  | No       | Identifier for the ingress. If omitted, a random ID is generated.                                        |
| `--auth-key` | string  | No       | Authentication key required to connect to the ingress. If omitted, a random key is generated and logged. |

### Client CLI

```bash
siffleux client [CLIENT_OPTIONS] [EGRESS]
```

#### Client Options

| Option     | Type | Required | Description                               |
| ---------- | ---- | -------- | ----------------------------------------- |
| `--config` | path | No       | Path to a TOML client configuration file. If configured, all the other arguments won't be considered. |

#### `tcp`

```bash
siffleux client tcp \
    --server <SERVER> \
    --cert-hash <CERT_HASH> \
    --ingress-id <INGRESS_ID> \
    --auth-key <AUTH_KEY> \
    --target <TARGET>
```

| Option                           | Type   | Required | Description                                                                                          |
| -------------------------------- | ------ | -------- | ---------------------------------------------------------------------------------------------------- |
| `--server`                       | string | Yes      | Address (`hostname:port` or `ip:port`) of the server to connect to.                                  |
| `--cert-hash`             | string | Yes      | Expected hash of the server's TLS certificate, used for validation.                                  |
| `--ingress-id`                   | string | Yes      | ID of the server-side TCP ingress this egress attaches to.                                           |
| `--auth-key`                     | string | Yes      | Authentication key used to authenticate with the target ingress.                                     |
| `--target`                       | string | Yes      | Address (`hostname:port` or `ip:port`) of the target service to forward the TCP connections to.      |
| `--id`                           | string | No       | Identifier for this egress. If omitted, a random ID is generated.                                    |
| `--cert-subject-alt-name` | string | No       | Expected Subject Alternative Name on the server's certificate. Defaults to `self-host.siffleux.dev`. |

#### `udp`

```bash
siffleux client udp \
    --server <SERVER> \
    --cert-hash <CERT_HASH> \
    --ingress-id <INGRESS_ID> \
    --auth-key <AUTH_KEY> \
    --target <TARGET>
```

| Option                           | Type   | Required | Description                                                                                          |
| -------------------------------- | ------ | -------- | ---------------------------------------------------------------------------------------------------- |
| `--server`                       | string | Yes      | Address (`hostname:port` or `ip:port`) of the server to connect to.                                  |
| `--cert-hash`             | string | Yes      | Expected hash of the server's TLS certificate, used for validation.                                  |
| `--ingress-id`                   | string | Yes      | ID of the server-side UDP ingress this egress attaches to.                                           |
| `--auth-key`                     | string | Yes      | Authentication key used to authenticate with the target ingress.                                     |
| `--target`                       | string | Yes      | Address (`hostname:port` or `ip:port`) of the target service to forward the UDP datagrams to.        |
| `--id`                           | string | No       | Identifier for this egress. If omitted, a random ID is generated.                                    |
| `--cert-subject-alt-name` | string | No       | Expected Subject Alternative Name on the server's certificate. Defaults to `self-host.siffleux.dev`. |
