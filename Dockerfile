# syntax=docker/dockerfile:1
ARG OPENC3_TAG=7.4.0

# Stage 1: build static oms_bridge binary (musl, no shared libs)
FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY config/CALConfig.toml config/oms_field_mappings.toml ./config/
COPY .cargo ./.cargo
COPY build.rs ./
COPY src ./src
COPY local_deps ./local_deps
# Cargo.toml references rcal at ../rcal/rcal (i.e. /rcal/rcal from /build)
RUN mkdir -p /rcal && ln -s /build/local_deps/rcal /rcal/rcal
RUN cargo build --release --target x86_64-unknown-linux-musl -p oms_bridge

# Stage 2: build the cosmos plugin gem
FROM ruby:3-alpine AS gem-builder

WORKDIR /plugin
COPY cosmos-oms.gemspec plugin.txt ./
COPY microservices ./microservices
# Pull the generated telemetry definitions and musl binary from the Rust build.
COPY --from=builder /build/targets/OMS/cmd_tlm/oms_generated.txt ./targets/OMS/cmd_tlm/
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/oms_bridge ./microservices/OMS_BRIDGE/bin/oms_bridge
RUN gem build cosmos-oms.gemspec

# Stage 3: extend the standard operator image with the binary and gem
FROM openc3inc/openc3-operator:${OPENC3_TAG}

COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/oms_bridge /usr/local/bin/oms_bridge
COPY --from=gem-builder /plugin/cosmos-oms-1.0.6.gem /cosmos-oms.gem
COPY plugin_instance.json /plugin_instance.json
