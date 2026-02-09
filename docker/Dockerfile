# ── Stage 1: Build ────────────────────────────────────────────
FROM rust:1.75-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/ crates/

# Build only the server binary
RUN cargo build --release --bin liberte-server

# ── Stage 2: Runtime ──────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Run as non-root
RUN groupadd -r liberte && useradd -r -g liberte -d /data -s /sbin/nologin liberte

COPY --from=builder /app/target/release/liberte-server /usr/local/bin/

RUN mkdir -p /data/blobs && chown -R liberte:liberte /data

USER liberte

EXPOSE 4001/udp
EXPOSE 8080

ENV RUST_LOG=liberte=info,libp2p=warn
ENV LISTEN_ADDR=/ip4/0.0.0.0/udp/4001/quic-v1
ENV HTTP_ADDR=0.0.0.0:8080
ENV BLOB_STORAGE_PATH=/data/blobs

VOLUME ["/data"]

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD curl -sf http://localhost:8080/health || exit 1

CMD ["liberte-server"]
