# syntax=docker/dockerfile:1.7
#
# Single Dockerfile for all five Rust backend images.
#
# The five binaries share one workspace, so they share one `builder`
# stage that compiles everything exactly once. Each service then has
# a tiny final stage (selected via `target:` in docker-compose.yml)
# that copies just its binary into the slim runtime base.
#
# Why one file instead of five: `docker compose build` hands all the
# targets to BuildKit in a single bake session, which deduplicates
# identical stages — the dependency cook and the workspace build run
# once, not five times in parallel. On memory-constrained hosts
# (7 GB WSL2) five concurrent rustc trees OOM-kill the compiler; this
# layout makes a plain `docker compose --profile backend up --build`
# safe, and ~5x faster on cold cache.
#
# Multi-stage build:
#   chef         — base image with cargo-chef installed.
#   planner      — produces a recipe.json describing the dep graph.
#   builder      — cooks the deps (cached on Cargo.lock), then builds
#                  all five binaries against the real source tree.
#   runtime      — slim Debian base shared by every final stage.
#   yog-*        — one per service: COPY its binary, set ENTRYPOINT.
#
# Build context: repo root. The workspace's Cargo.toml, Cargo.lock
# and rust-toolchain.toml must all be visible.

# ── chef base ──────────────────────────────────────────────────────
FROM rust:1.95-slim-bookworm AS chef
WORKDIR /app

# pkg-config + libssl-dev are needed by several transitive crates
# (rustls is pure-Rust but openssl-sys still slips in via solana
# transitive deps in some configurations).
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# cargo-chef pinned to a known-good release. Locked install avoids
# pulling new transitive versions on every image build.
RUN cargo install cargo-chef --locked --version 0.1.71

# ── planner ────────────────────────────────────────────────────────
# Copies the manifests + source just long enough to compute the
# recipe. The recipe is independent of the source content, so this
# stage is cheap; only its output (recipe.json) is consumed below.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── builder ────────────────────────────────────────────────────────
FROM chef AS builder

# sqlx::query!() macros are validated at compile time. Offline mode
# tells them to read the committed empreintes under
# crates/persistence/.sqlx/ instead of opening a connection.
ENV SQLX_OFFLINE=true

# Cook the full workspace dep graph based on the recipe alone. As
# long as Cargo.lock does not change, this layer is reused —
# modifying an .rs file does NOT invalidate it.
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Now bring in the real source and build every binary in one cargo
# invocation. Only the workspace code is (re)compiled at this point;
# the dep graph is already cached from `chef cook`.
COPY . .
RUN cargo build --release \
        --bin yog-migrate \
        --bin yog-indexer \
        --bin yog-api \
        --bin yog-context \
        --bin yog-signals

# ── runtime base ───────────────────────────────────────────────────
# libssl3 covers any non-rustls TLS code paths transitively pulled
# in by Solana / sqlx-postgres. ca-certificates is required for
# any HTTPS the binary opens (RPC endpoints).
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system yog \
    && useradd --system --gid yog --home-dir /app --shell /usr/sbin/nologin yog

WORKDIR /app
USER yog

# ── final stages (pick with `target:` in docker-compose.yml) ───────

# yog-migrate — one-shot DDL applier. Runs once per `docker compose
# up`, exits 0 when the schema is in sync; the other backend services
# depend on it via `service_completed_successfully`.
FROM runtime AS yog-migrate
COPY --from=builder /app/target/release/yog-migrate /usr/local/bin/yog-migrate
ENTRYPOINT ["/usr/local/bin/yog-migrate"]

# yog-indexer — Solana WebSocket ingestion daemon.
FROM runtime AS yog-indexer
COPY --from=builder /app/target/release/yog-indexer /usr/local/bin/yog-indexer
# Prometheus /metrics endpoint. Documented as an EXPOSE for human
# readers; the actual port mapping is decided by docker-compose.
EXPOSE 9000
ENTRYPOINT ["/usr/local/bin/yog-indexer"]

# yog-api — axum HTTP server.
FROM runtime AS yog-api
COPY --from=builder /app/target/release/yog-api /usr/local/bin/yog-api
EXPOSE 5000
ENTRYPOINT ["/usr/local/bin/yog-api"]

# yog-context — token enrichment daemon (metadata + price).
FROM runtime AS yog-context
COPY --from=builder /app/target/release/yog-context /usr/local/bin/yog-context
EXPOSE 9000
ENTRYPOINT ["/usr/local/bin/yog-context"]

# yog-signals — the Signal Engine daemon.
FROM runtime AS yog-signals
COPY --from=builder /app/target/release/yog-signals /usr/local/bin/yog-signals
EXPOSE 9000
ENTRYPOINT ["/usr/local/bin/yog-signals"]
