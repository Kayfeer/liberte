# syntax=docker/dockerfile:1
FROM rust:1.99.1-bookworm AS builder

WORKDIR /app
# A missing lockfile is a hard failure; CI can produce the initial candidate.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/ crates/
RUN cargo build --release --locked --bin liberte-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system liberte \
    && useradd --system --gid liberte --home-dir /data --shell /usr/sbin/nologin liberte \
    && mkdir -p /data/blobs \
    && chown -R liberte:liberte /data

COPY --from=builder /app/target/release/liberte-server /usr/local/bin/liberte-server
USER liberte
EXPOSE 4001/udp 8080
ENV RUST_LOG=liberte=info,libp2p=warn \
    LISTEN_ADDR=/ip4/0.0.0.0/udp/4001/quic-v1 \
    HTTP_ADDR=0.0.0.0:8080 \
    BLOB_STORAGE_PATH=/data/blobs
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl --fail --silent http://localhost:8080/health || exit 1
CMD ["liberte-server"]

