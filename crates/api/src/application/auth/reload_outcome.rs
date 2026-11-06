/// How a reread of the key ring ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReloadOutcome {
    Reloaded,
    /// The read failed; the ring keeps the keys it held.
    Failed,
}

impl ReloadOutcome {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Reloaded => "reloaded",
            Self::Failed => "failed",
        }
    }
}
