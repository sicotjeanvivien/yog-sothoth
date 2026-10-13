//! The loop that writes one cycle per interval, until the stop.

mod log;
/// A namespace: read as `metrics::record_cycle`, `metrics::HEARTBEAT_FAILURES`.
pub(crate) mod metrics;
mod worker;

pub(crate) use worker::AnalyticWorker;
