FROM rust:1.97-slim AS builder

COPY . .

RUN cargo build --release --bin siffleux-cli

FROM gcr.io/distroless/cc-debian13

COPY --from=builder /target/release/siffleux-cli /usr/local/bin/siffleux

USER nonroot:nonroot

WORKDIR /siffleux

ENTRYPOINT ["/usr/local/bin/siffleux"]
