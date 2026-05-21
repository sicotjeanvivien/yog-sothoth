# syntax=docker/dockerfile:1.7
#
# yog-indexer — Solana WebSocket ingestion daemon.
#
# Multi-stage build:
#   chef    — base image with cargo-chef installed.
#   planner — produces a recipe.json describing the dep graph.
#   builder — cooks the deps (cached on Cargo.lock), then builds
#             the binary against the real source tree.
#   runtime — slim Debian, just the libs the binary dynamically
#             links against, plus the binary itself.
#
# Build context: repo root (not crates/indexer/). The workspace's
# Cargo.toml, Cargo.lock and rust-toolchain.toml must all be visible.

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

# Cook the deps based on the recipe alone. As long as Cargo.lock
# does not change, this layer is reused — modifying an .rs file
# does NOT invalidate it.
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin yog-indexer

# Now bring in the real source and build the binary. Only the
# workspace code is (re)compiled at this point; the dep graph is
# already cached from `chef cook`.
COPY . .
RUN cargo build --release --bin yog-indexer

# ── runtime ────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# libssl3 covers any non-rustls TLS code paths transitively pulled
# in by Solana / sqlx-postgres. ca-certificates is required for
# any HTTPS the binary opens (RPC endpoints).
RUN apt-get update && apt-get install -y --no-install-recommends \
        libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system yog \
    && useradd --system --gid yog --home-dir /app --shell /usr/sbin/nologin yog

WORKDIR /app
COPY --from=builder /app/target/release/yog-indexer /usr/local/bin/yog-indexer

USER yog

# Prometheus /metrics endpoint. Documented as an EXPOSE for human
# readers; the actual port mapping is decided by docker-compose.
EXPOSE 9000

ENTRYPOINT ["/usr/local/bin/yog-indexer"]
