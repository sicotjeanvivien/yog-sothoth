# syntax=docker/dockerfile:1.7
#
# yog-context — token enrichment daemon (metadata + price).
#
# Same multi-stage shape as the other backend Dockerfiles. No EXPOSE
# directive: yog-context is a pure daemon with no inbound traffic.

FROM rust:1.95-slim-bookworm AS chef
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
  pkg-config libssl-dev ca-certificates \
  && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked --version 0.1.71

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ENV SQLX_OFFLINE=true
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin yog-context
COPY . .
RUN cargo build --release --bin yog-context

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
  libssl3 ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && groupadd --system yog \
  && useradd --system --gid yog --home-dir /app --shell /usr/sbin/nologin yog

WORKDIR /app
COPY --from=builder /app/target/release/yog-context /usr/local/bin/yog-context

USER yog

# Prometheus /metrics endpoint. Documented as an EXPOSE for human
# readers; the actual port mapping is decided by docker-compose.
EXPOSE 9000

ENTRYPOINT ["/usr/local/bin/yog-context"]
