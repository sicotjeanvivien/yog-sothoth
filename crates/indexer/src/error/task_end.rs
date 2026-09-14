use tokio::task::JoinError;

/// What a [`JoinError`] actually says.
///
/// `tokio` bundles two outcomes in one type and only one of them is a failure:
/// the task panicked, or it was cancelled. Reading a `JoinError` as a panic
/// reports a normal stop as a crash — and, worse, counts it as one.
///
/// ⚠️ **One type, because it is one rule, applied at two sites**: the fleet's
/// join loop in `infra::rpc::listener` and the daemon's own task results. A
/// rule written twice is a rule that holds at one site out of two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskEnd {
    /// The task's future panicked. A real failure, and the only one here.
    Panicked,
    /// The task was destroyed before it could finish — aborted, or caught by
    /// the runtime shutting down. Nothing failed; the work was cut short.
    Cancelled,
}

impl From<&JoinError> for TaskEnd {
    fn from(error: &JoinError) -> Self {
        if error.is_cancelled() {
            Self::Cancelled
        } else {
            Self::Panicked
        }
    }
}
