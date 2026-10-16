use thiserror::Error;

/// Failure modes of one network status tick: counted by `NetworkStatusReporter::tick`, never
/// propagated; typed so the boundary says what went wrong.
#[derive(Debug, Error)]
pub(crate) enum NetworkStatusReporterError {
    /// The `getSlot` RPC call failed.
    #[error("network status reporter: getSlot RPC call failed: {0}")]
    Rpc(String),

    /// Persisting the snapshot failed; the cause is in the message only.
    /// ⚠️ No `#[from]`: as `source()` too, a chain-walking formatter would print it twice.
    #[error("network status reporter: failed to persist snapshot: {0}")]
    Persistence(yog_core::RepositoryError),
}

impl NetworkStatusReporterError {
    /// The `reason` label of `yog_indexer_network_status_tick_failures_total`.
    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::Rpc(_) => "rpc",
            Self::Persistence(_) => "persistence",
        }
    }
}
