FROM rust:1.93.0-bookworm AS builder
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libsqlite3-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY server/Cargo.toml server/Cargo.toml
COPY server/src server/src
RUN cargo build --release -p bambu-lan-viewer-backend

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/bambu-lan-viewer-backend /usr/local/bin/bambu-lan-viewer-backend
RUN mkdir -p /data /hls

ENV DATABASE_URL=sqlite:///data/printers.db \
    HLS_OUTPUT_DIR=/hls \
    HTTP_BIND=0.0.0.0:8080

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/bambu-lan-viewer-backend"]
