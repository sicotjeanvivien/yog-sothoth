//! The Yellowstone gRPC ingestion path, chosen by `INGEST_SOURCE=grpc` in `init_source`.
//! In data order: `listener` (connection, retries), `subscription` (the request), `session` (one
//! update, and the slots it received for good), `slot_timestamp_buffer` (a slot's `block_time`),
//! `transaction_adapter`; `source` is public.
//! ⚠️ Tested only against a scripted server (`test_geyser_server`): TLS, keep-alive, ping answers
//! and `from_slot` semantics are unproven until a run against a real provider.

mod interceptor;
mod listener;
mod metrics;
mod ranges_to_record;
mod session;
mod slot_timestamp_buffer;
mod source;
mod subscription;
mod transaction_adapter;

// ⚠️ `#[path]` resolves from the declaring file's directory — `infra/` here, hence the `grpc/`;
// `grpc/` and `rpc/` files need only `tests/`, `grpc/listener/` files `../tests/`.
#[cfg(test)]
#[path = "grpc/tests/fixtures.rs"]
mod test_fixtures;
#[cfg(test)]
#[path = "grpc/tests/geyser_server.rs"]
mod test_geyser_server;

pub(crate) use listener::{GrpcListener, RECORD_PERIOD, STALL_TIMEOUT, SlotsRecorder};
pub(crate) use metrics::{GrpcBufferMetrics, GrpcListenerMetrics};
pub(crate) use source::GrpcTransactionSource;
