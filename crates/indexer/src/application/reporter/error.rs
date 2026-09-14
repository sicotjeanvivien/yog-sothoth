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
    /// The cause is in the message, and **only** there: the error ends in a
    /// `warn!` that prints `Display`, where "failed to persist" without the why
    /// is a line nobody can act on. No `#[from]`, which would also make it the
    /// `source()` and print it twice under any chain-walking formatter — the
    /// one call site maps explicitly instead.
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
