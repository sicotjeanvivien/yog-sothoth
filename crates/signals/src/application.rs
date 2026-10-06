//! What the daemon does once wired: the detectors' rules, and the loops that
//! run on their own cadence until the stop — each with what only it uses.

pub(crate) mod detectors;
pub(crate) mod workers;

/// Shared harness for the tests that assert on a metric — a skip *counted*,
/// not merely not-signalled; a gauge brought back to zero.
#[cfg(test)]
mod metrics_probe;
