//! What the daemon does once wired: one archiving run, and the loop that
//! repeats it until the stop.

mod archiver;
/// A namespace: read as `metrics::record`, `metrics::HEARTBEAT_FAILURES`.
pub(crate) mod metrics;
mod run_outcome;
mod stream;
mod worker;

pub(crate) use archiver::{Archiver, VersionSource};
pub(crate) use worker::ArchiveWorker;
