//! The Yellowstone gRPC ingestion path.
//!
//! Sibling of [`super::rpc`]. Reading order, which is also the order the data
//! travels:
//!
//! - `listener` — the connection, the subscription, and what to do when the
//!   stream breaks;
//! - `subscription` — what is asked for, and how an update names its protocol;
//! - `session` — what one update does: route it, pair it with its slot's time,
//!   translate it, hand it downstream;
//! - `interceptor` — the credential, on every request, printed by nothing;
//! - `transaction_adapter` — the protobuf shape into the neutral transaction;
//! - `slot_timestamp_buffer` — the pairing itself, since `block_time` lives on
//!   a separate subscription keyed by slot;
//! - `source` — the port's face on all of it, and the only public item.
//!
//! What leaves is `application::source::IngestedTransaction`, which is not this
//! path's type: it is the port's, and this path was merely the first to need
//! it.
//!
//! Selected by `INGEST_SOURCE=grpc`, through `bootstrap/daemon.rs::init_source`
//! — the only place in the crate that names a concrete source. Everything
//! downstream of `source` sees the port and never learns which model is
//! running.
//!
//! ⚠️ **Nothing here is exercised against a real server.** The connection, TLS,
//! the retry budget, keep-alive and the exact semantics of `from_slot` are
//! written, reviewed and unproven; `02 - backlog/pre-v02/flux-grpc-reel-mesures.md`
//! is where they meet one. What *is* testable was deliberately kept out of
//! `listener`: the request in `subscription`, the meaning of each update in
//! `session`, the pairing in `slot_timestamp_buffer`, the shape in
//! `transaction_adapter`.
//!
//! The `#![allow(dead_code)]` this module carried until 10 September 2026 is
//! gone with the wiring it was waiting for. It had covered the whole path
//! rather than each module, precisely so that deleting it would make the build
//! name whatever was still unreachable — which it did.

mod interceptor;
mod listener;
mod metrics;
mod session;
mod slot_timestamp_buffer;
mod source;
mod subscription;
mod transaction_adapter;

pub(crate) use listener::GrpcListener;
pub(crate) use metrics::{GrpcBufferMetrics, GrpcListenerMetrics};
pub(crate) use source::GrpcTransactionSource;
