//! The Yellowstone gRPC ingestion path.
//!
//! Sibling of [`super::rpc`], and complete except for being chosen. Reading
//! order, which is also the order the data travels:
//!
//! - `listener` — the connection, the subscription, and what to do when the
//!   stream breaks;
//! - `subscription` — what is asked for, and how an update names its protocol;
//! - `session` — what one update does: route it, pair it with its slot's time,
//!   translate it, hand it downstream;
//! - `interceptor` — the credential, on every request, printed by nothing;
//! - `transaction_adapter` — the protobuf shape into the neutral transaction;
//! - `slot_timestamp_buffer` — the pairing itself, since `block_time` lives on
//!   a separate subscription keyed by slot.
//!
//! What leaves is `application::source::IngestedTransaction`, which is not this
//! path's type: it is the port's, and this path was merely the first to need
//! it.
//!
//! ⚠️ **Nothing selects this path.** What the listener emits has no consumer,
//! and `INGEST_SOURCE=grpc` is still refused by `check_supported` — the fourth
//! slice of `03 - active/listener-grpc-yellowstone.md` is what lifts both.
//! Two things here are live already, and deliberately: the metric families
//! below, registered by `bootstrap/daemon.rs` whichever source runs, and
//! `bootstrap/config.rs` reading `INGEST_STREAM` through the
//! `required_endpoint_allowing_header` door — whatever the source, since both
//! listeners send the header the operator declares (`infra::credential`).
//!
//! Being unreachable, the rest would trip `dead_code` under `-D warnings`.
//! Hence the single `allow` below — one for the whole path rather than one per
//! module, and one line to delete when the switch lands, at which point the
//! build names whatever is still unreachable. Not the `_`-prefix convention
//! `RpcListener::_watch` uses: that one marks a lone item among live
//! neighbours.

#![allow(dead_code)]

mod interceptor;
mod listener;
mod metrics;
mod session;
mod slot_timestamp_buffer;
mod subscription;
mod transaction_adapter;

pub(crate) use metrics::{GrpcBufferMetrics, GrpcListenerMetrics};
