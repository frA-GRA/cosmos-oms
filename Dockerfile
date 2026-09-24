# syntax=docker/dockerfile:1
FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY oms_bridge/Cargo.toml ./oms_bridge/

# Fetch dependencies before copying source (cache layer)
COPY .cargo ./.cargo
RUN cargo fetch --locked

# Copy source and build
COPY oms_bridge/src ./oms_bridge/src

RUN RCAL_XSD_PATH=$(find "$CARGO_HOME" -name 'UCI_Minimal_v2_5_0.xsd' -print -quit) \
    cargo build --release --locked --target x86_64-unknown-linux-musl -p oms_bridge

FROM scratch
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/oms_bridge /oms_bridge
COPY plugin.txt /plugin.txt
COPY targets /targets
COPY microservices /microservices

EXPOSE 7779
ENTRYPOINT ["/oms_bridge"]
