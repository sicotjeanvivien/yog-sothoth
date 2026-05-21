# syntax=docker/dockerfile:1.7
#
# yog-migrate — one-shot DDL applier.
#
# Same shape as the runtime services. Runs once per `docker compose
# up`, exits 0 when the schema is in sync. The other backend
# services depend on it via `service_completed_successfully`.

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
RUN cargo chef cook --release --recipe-path recipe.json --bin yog-migrate
COPY . .
RUN cargo build --release --bin yog-migrate

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system yog \
    && useradd --system --gid yog --home-dir /app --shell /usr/sbin/nologin yog

WORKDIR /app
COPY --from=builder /app/target/release/yog-migrate /usr/local/bin/yog-migrate

USER yog

ENTRYPOINT ["/usr/local/bin/yog-migrate"]
