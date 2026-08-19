# syntax=docker/dockerfile:1

# ── Build stage ──────────────────────────────────────────────────────────
FROM rust:1.97-bookworm AS builder

WORKDIR /build

# Copy manifests and sources. (A dependency-only cache layer is not used
# because the workspace is small; add one here if build times grow.)
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates

RUN cargo build --release --bin parallel-research

# ── Runtime stage ────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 researcher

COPY --from=builder /build/target/release/parallel-research /usr/local/bin/parallel-research

USER researcher
WORKDIR /data

# Research output and the SQLite database live in /data; the config file is
# read from /home/researcher/.parallel-research/config.toml. Mount volumes
# for both (see docker-compose.yml).
ENV RUST_LOG=info
VOLUME ["/data"]

EXPOSE 8080

ENTRYPOINT ["parallel-research"]
CMD ["serve", "--port", "8080"]
