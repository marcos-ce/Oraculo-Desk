FROM docker.io/library/rust:1-slim-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ENV CARGO_HOME=/cargo-cache \
    CARGO_TARGET_DIR=/target-cache

WORKDIR /work
