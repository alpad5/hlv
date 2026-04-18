# ---- Build stage ----
# Use the official Rust image to compile a release binary.
FROM rust:1.85-slim AS builder

WORKDIR /app

# Copy only the backend source (build context is the repo root).
COPY backend/ .

RUN cargo build --release

# ---- Runtime stage ----
# Minimal Debian image — no Rust toolchain, just the binary and TLS certs.
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/hlv /usr/local/bin/hlv

CMD ["hlv"]
