use thiserror::Error;

/// Failure modes of one network status tick.
///
/// **Counted by `NetworkStatusReporter::tick`, never propagated.** The
/// reporter is an observer, not a pipeline stage: a failed tick is logged,
/// counted under [`reason`](Self::reason), and the next tick tries again. The
/// type stays typed so the boundary says what went wrong, not so it can travel.
#[derive(Debug, Error)]
pub(crate) enum NetworkStatusReporterError {
    /// The `getSlot` RPC call failed (RPC unreachable, transport
    /// error, malformed response).
    #[error("network status reporter: getSlot RPC call failed: {0}")]
    Rpc(String),

    /// Persisting the snapshot failed. Wraps the repository error.
    ///
    /// The cause is in the message, not only in `source()`: the error now ends
    /// in a `warn!` that prints `Display`, and "failed to persist" without the
    /// why is a line nobody can act on.
    #[error("network status reporter: failed to persist snapshot: {0}")]
    Persistence(#[from] yog_core::RepositoryError),
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
