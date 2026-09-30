//! How a run ended, and why a failed one failed.

/// How a run ended: a dump in the bucket, a stop, or a failure. Three
/// cases, and every `match` on it — the signal, the log, the metrics — has
/// exactly these three arms and no catch-all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunOutcome {
    /// A dump is in the bucket under `key`.
    Archived { key: String, bytes: u64 },
    /// The process is stopping. Not a failure, and not signalled: the next
    /// start dumps at once.
    Cancelled,
    /// Signalled, with its reason.
    Failed(RunFailure),
}

impl RunOutcome {
    /// The label used in metrics and in the failure signal.
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Archived { .. } => "archived",
            Self::Cancelled => "cancelled",
            Self::Failed(failure) => failure.kind.label(),
        }
    }
}

/// Why a run failed: the kind names it, the reason says what happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunFailure {
    pub(crate) kind: FailureKind,
    pub(crate) reason: String,
}

/// Where a run failed. A new kind is signalled as a failure without anything
/// else to decide — which is the right default for a backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailureKind {
    /// No dump was attempted: the versions could not be read, or `pg_dump`
    /// is not the server's major.
    Refused,
    /// `pg_dump` could not start, failed, or its output could not be read.
    DumpFailed,
    /// `pg_dump` succeeded but `pg_restore` cannot read what it produced.
    Unreadable,
    /// The bucket refused the upload, one of its parts, or its completion.
    StoreFailed,
}

impl FailureKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::DumpFailed => "dump_failed",
            Self::Unreadable => "unreadable",
            Self::StoreFailed => "store_failed",
        }
    }
}

impl RunFailure {
    fn new(kind: FailureKind, reason: impl std::fmt::Display) -> Self {
        Self {
            kind,
            reason: reason.to_string(),
        }
    }

    pub(crate) fn refused(reason: impl std::fmt::Display) -> Self {
        Self::new(FailureKind::Refused, reason)
    }

    pub(crate) fn dump_failed(reason: impl std::fmt::Display) -> Self {
        Self::new(FailureKind::DumpFailed, reason)
    }

    pub(crate) fn unreadable(reason: impl std::fmt::Display) -> Self {
        Self::new(FailureKind::Unreadable, reason)
    }

    pub(crate) fn store_failed(reason: impl std::fmt::Display) -> Self {
        Self::new(FailureKind::StoreFailed, reason)
    }
}
