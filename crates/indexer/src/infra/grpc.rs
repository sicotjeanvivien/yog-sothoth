//! The Yellowstone gRPC ingestion path.
//!
//! Sibling of [`super::rpc`], and deliberately incomplete: this slice carries
//! the schema adapter alone. Nothing calls it yet — the listener that will is
//! the third slice of `03 - active/listener-grpc-yellowstone.md`, and
//! `INGEST_SOURCE=grpc` stays refused at startup until the fourth.
//!
//! Being unreachable, the adapter would trip `dead_code` under `-D warnings`.
//! It carries a single module-level `allow` with its reason rather than the
//! `_`-prefix convention `RpcListener::_watch` uses: that one marks a lone item
//! among live neighbours, whereas here the entry point *and* its six helpers are
//! dead together. One line to delete when wiring, not seven prefixes to strip.

mod transaction_adapter;
