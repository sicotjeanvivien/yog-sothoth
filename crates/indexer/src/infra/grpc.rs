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
//! Selected by `INGEST_SOURCE=grpc`, through
//! `bootstrap/daemon/init.rs::init_source` — the only place in the crate that
//! names a concrete source. Everything
//! downstream of `source` sees the port and never learns which model is
//! running.
//!
//! ⚠️ **Nothing here has met a real server.** What changed on 16 September 2026
//! is that the retry rule now meets a *scripted* one: `test_geyser_server` serves a
//! test-written script over loopback so `listener_tests` can drive
//! `GrpcListener::run` through every ending a stream can have. The one ending
//! that is **not** a stream's — the attempt that never got an answer — is
//! guarded in two halves: what it costs the retry budget goes through `run`
//! like the others, against a port with nothing behind it, while what it does
//! to the resume mark is driven one level down, at `connect_and_stream`,
//! because no server can both deliver a mark and be unreachable for the attempt
//! after. `listener.rs`'s header carries the measurements. All of it proves this
//! client against our model of the server, and nothing about the protocol — so
//! TLS, keep-alive and the exact semantics of `from_slot` are still written,
//! reviewed and unproven, and `02 - backlog/[spike]flux-grpc-reel-mesures.md`
//! is still where they meet one. The rest was made testable by being kept out
//! of `listener`: the request in `subscription`, the meaning of each update in
//! `session`, the pairing in `slot_timestamp_buffer`, the shape in
//! `transaction_adapter`.
//!
//! The last two modules below are `#[cfg(test)]` and carry no production code:
//! `test_fixtures` builds the protobuf updates both test modules send, and
//! `test_geyser_server` is the scripted server itself.
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

// Test-only, and last so that the list above is the path itself. They live in
// `grpc/tests/` with the six `_tests.rs` files, which is what a directory
// listing needs to say; the module keeps the `test_` prefix, which is what a
// `use` needs to say. Same split as `api`'s `request.rs` — file `common.rs`,
// module `test_common`.
//
// ⚠️ **The `grpc/` in these paths is not a typo.** `#[path]` resolves against
// the directory of the *declaring file*, and this file is `infra/grpc.rs`, so
// the base is `infra/`. The thirteen `#[path]`s inside `grpc/` and `rpc/` are
// already one level down and need only `tests/`.
#[cfg(test)]
#[path = "grpc/tests/fixtures.rs"]
mod test_fixtures;
#[cfg(test)]
#[path = "grpc/tests/geyser_server.rs"]
mod test_geyser_server;

pub(crate) use listener::GrpcListener;
pub(crate) use metrics::{GrpcBufferMetrics, GrpcListenerMetrics};
pub(crate) use source::GrpcTransactionSource;
