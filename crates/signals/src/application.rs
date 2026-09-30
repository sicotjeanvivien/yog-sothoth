//! What the daemon does once wired: the rules — the detectors', and the
//! materialisation alarm's — and the loops that apply them on their own
//! cadence until the stop.

pub(crate) mod detectors;
pub(crate) mod materialization;
pub(crate) mod workers;

/// Shared harness for the tests that assert on a metric — a skip *counted*,
/// not merely not-signalled; a gauge brought back to zero.
#[cfg(test)]
mod metrics_probe;
