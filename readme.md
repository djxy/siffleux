<div align="center">
    <img src="./assets/logo.png" alt="Siffleux logo" width="200">
</div>

> Siffleux, pronounced siflø, is the French-Canadian name for a groundhog.

# Siffleux
Siffleux is a tunnelling software developed in Rust and [QUIC](https://en.wikipedia.org/wiki/QUIC). It allows services hosted behind a NAT or a firewall to be reachable without opening ingress ports.

## How it works
Siffleux works by establishing a persistent QUIC connection between a client (running inside your private network) and a server (running on a reachable host). Because the client initiates the connection to the server, no ingress ports need to be opened on the NAT or the firewall. Once connected, the server forwards all incoming connections to the client through the tunnel. The client relays them to the services in your private network.

You define the ingress endpoints on the server and egress endpoints on the client. Each ingress can be paired with one or multiple egresses for load balancing.

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

## Getting started

*Coming soon*
