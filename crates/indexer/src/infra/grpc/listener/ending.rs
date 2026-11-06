//! How one connection ended, and what that ending is worth.

/// How one connection ended. Stream endings carry `delivered` (whether data came off the
/// stream); what an ending is worth is [`Ending::verdict`]'s alone.
pub(super) enum Ending {
    ShutdownRequested,
    DownstreamClosed,
    /// The attempt never got an answer from the service: the dial, `subscribe`
    /// failing on **our** transport (see `reached_the_service`), or the
    /// outbound half, which nothing reaches today.
    Unreachable {
        error: String,
    },
    StreamClosed {
        delivered: bool,
    },
    Failed {
        error: String,
        delivered: bool,
    },
    /// The server said nothing for [`STALL_TIMEOUT`](super::stall_clock::STALL_TIMEOUT), and this
    /// listener ended the attempt.
    Stalled {
        error: String,
        delivered: bool,
    },
}

/// What an ending is worth — decided once, read by the retry budget.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// A stop was requested.
    ShutdownRequested,
    /// The consumer is gone: nothing will take a transaction again.
    ConsumerGone,
    /// The stream delivered before it ended. `error` is `None` for a clean
    /// close, and says what broke or stalled otherwise.
    Delivered { error: Option<String> },
    /// The server answered and gave nothing back: a refusal, credentials, quota, an incident, or
    /// a close with nothing sent.
    ServerFailed { error: String },
    /// Nobody answered the request: the service was unreachable, or silent.
    Unanswered { error: String },
}

impl Ending {
    /// What this ending is worth.
    pub(super) fn verdict(self) -> Verdict {
        match self {
            Ending::ShutdownRequested => Verdict::ShutdownRequested,
            Ending::DownstreamClosed => Verdict::ConsumerGone,
            Ending::Unreachable { error } => Verdict::Unanswered { error },

            Ending::StreamClosed { delivered: true } => Verdict::Delivered { error: None },
            Ending::StreamClosed { delivered: false } => Verdict::ServerFailed {
                error: "stream closed without delivering anything".to_string(),
            },

            Ending::Failed {
                error,
                delivered: true,
            } => Verdict::Delivered { error: Some(error) },
            Ending::Failed {
                error,
                delivered: false,
            } => Verdict::ServerFailed { error },

            Ending::Stalled {
                error,
                delivered: true,
            } => Verdict::Delivered { error: Some(error) },
            Ending::Stalled {
                error,
                delivered: false,
            } => Verdict::Unanswered { error },
        }
    }
}
