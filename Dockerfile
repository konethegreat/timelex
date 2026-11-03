# syntax=docker/dockerfile:1
# Written by Kone Tshivhinda & Claude
# One image carries the Phase 1 services and the Phase 2 capture hub; compose picks the binary with
# TIMELEX_BINARY.

FROM rust:1-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY services ./services
COPY tools ./tools
# Embedded at compile time: bootstrap SQL (timelex-db) and the MCP routes (gateway).
COPY contracts ./contracts
COPY deploy/postgres/bootstrap ./deploy/postgres/bootstrap
# The target cache speeds up rebuilds, but cache mounts never enter the image layer, so the binaries
# are copied out to /out in the same step.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release \
        -p timelex-gateway \
        -p timelex-identity-service \
        -p timelex-vault-service \
        -p timelex-capture-service \
        -p timelex-matters-service \
    && mkdir -p /out \
    && cp target/release/timelex-gateway \
          target/release/timelex-identity-service \
          target/release/timelex-vault-service \
          target/release/timelex-capture-service \
          target/release/timelex-matters-service \
          /out/

FROM debian:bookworm-slim AS services
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /out/timelex-gateway /usr/local/bin/timelex-gateway
COPY --from=builder /out/timelex-identity-service /usr/local/bin/timelex-identity-service
COPY --from=builder /out/timelex-vault-service /usr/local/bin/timelex-vault-service
COPY --from=builder /out/timelex-capture-service /usr/local/bin/timelex-capture-service
COPY --from=builder /out/timelex-matters-service /usr/local/bin/timelex-matters-service
COPY deploy/container/entrypoint.sh /usr/local/bin/entrypoint
COPY deploy/container/generate-keys.sh /usr/local/bin/generate-keys
RUN chmod 0755 /usr/local/bin/entrypoint /usr/local/bin/generate-keys
ENTRYPOINT ["/usr/local/bin/entrypoint"]
USER 10001:10001
