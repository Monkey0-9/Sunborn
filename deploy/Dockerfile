# syntax=docker/dockerfile:1.4

# ==============================================================================
# Multi-Stage Production Dockerfile for Sunborn AI Gateway
# Target Binary: sunborn-gateway
# Architecture Invariants: Non-root execution, minimal surface (<35MB runtime)
# ==============================================================================

# ------------------------------------------------------------------------------
# Stage 1: Rust Builder
# ------------------------------------------------------------------------------
FROM rust:1.80-bullseye AS builder

WORKDIR /usr/src/sunborn

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy full workspace context
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps

# Build release binary for sunborn-gateway
RUN cargo build --release -p sunborn-gateway

# ------------------------------------------------------------------------------
# Stage 2: Distroless Minimal Non-Root Runtime
# ------------------------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

WORKDIR /app

# Copy stripped binary from builder
COPY --from=builder /usr/src/sunborn/target/release/sunborn-gateway /usr/local/bin/sunborn-gateway

# OCI labels
LABEL org.opencontainers.image.title="Sunborn AI Gateway" \
      org.opencontainers.image.description="Enterprise semantic caching and execution reuse platform for AI/RAG agents" \
      org.opencontainers.image.version="0.1.0" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.vendor="Sunborn Enterprise"

# Security: Non-root user (distroless:nonroot default uid is 65532)
USER 65532:65532

EXPOSE 8080

ENV RUST_LOG="info,sunborn_gateway=debug" \
    SUNBORN_SERVER__HOST="0.0.0.0" \
    SUNBORN_SERVER__PORT="8080"

ENTRYPOINT ["/usr/local/bin/sunborn-gateway"]
