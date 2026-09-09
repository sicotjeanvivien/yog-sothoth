//! The Yellowstone gRPC ingestion path.
//!
//! Sibling of [`super::rpc`], and deliberately incomplete. Two of the four
//! slices of `03 - active/listener-grpc-yellowstone.md` are here:
//!
//! - `transaction_adapter` — the protobuf shape into the neutral transaction;
//! - `slot_timestamp_buffer` — the pairing of that transaction with the block
//!   time the message does not carry, since `block_time` lives on a separate
//!   subscription keyed by slot.
//!
//! Nothing calls either yet: the listener that will is the third slice, and
//! `INGEST_SOURCE=grpc` stays refused at startup until the fourth.
//!
//! Being unreachable, all of it would trip `dead_code` under `-D warnings`.
//! Hence the single `allow` below — one for the whole path rather than one per
//! module, and one line to delete when the listener arrives. Not the
//! `_`-prefix convention `RpcListener::_watch` uses: that one marks a lone item
//! among live neighbours, and here nothing is live yet. The build says so the
//! moment that stops being true.

#![allow(dead_code)]

mod ingested_transaction;
mod interceptor;
mod listener;
mod metrics;
mod session;
mod slot_timestamp_buffer;
mod subscription;
mod transaction_adapter;

pub(crate) use metrics::{GrpcBufferMetrics, GrpcListenerMetrics};
