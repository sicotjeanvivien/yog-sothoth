//! How one connection ended, what that ending is worth, and what it does to the
//! resume mark.

/// How one connection ended.
///
/// The stream endings carry two facts: `resume_from`, where to pick up (see
/// `StreamSession::resume_from`), and `delivered`, whether any data came off
/// the stream. [`Ending::Unreachable`] carries neither: nothing reached the
/// service. What an ending is *worth* is [`Ending::verdict`]'s alone.
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
        resume_from: Option<u64>,
    },
    Failed {
        error: String,
        delivered: bool,
        resume_from: Option<u64>,
    },
    /// The server said nothing for [`STALL_TIMEOUT`], and this listener ended
    /// the attempt.
    ///
    /// [`STALL_TIMEOUT`]: super::stall_clock::STALL_TIMEOUT
    Stalled {
        error: String,
        delivered: bool,
        resume_from: Option<u64>,
    },
}

/// What an ending is worth — decided once, read by the two rules that follow
/// an attempt: the resume mark ([`Verdict::next_resume_from`]) and the retry
/// budget (`RetryBudget::settle`).
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// A stop was requested.
    ShutdownRequested,
    /// The consumer is gone: nothing will take a transaction again.
    ConsumerGone,
    /// The stream delivered before it ended. `error` is `None` for a clean
    /// close, and says what broke or stalled otherwise.
    Delivered {
        mark: Option<u64>,
        error: Option<String>,
    },
    /// The server had our `from_slot` and gave nothing back.
    Refused { error: String },
    /// Nobody answered the request: the service was unreachable, or silent.
    Unanswered { error: String },
}

impl Ending {
    /// What this ending is worth.
    ///
    /// ⚠️ **A stall is no verdict.** A `Failed` that delivered nothing is a
    /// refusal — the server had our `from_slot` — but a stall that delivered
    /// nothing is silence, and the decision to stop waiting is ours: it keeps
    /// the mark.
    pub(super) fn verdict(self) -> Verdict {
        match self {
            Ending::ShutdownRequested => Verdict::ShutdownRequested,
            Ending::DownstreamClosed => Verdict::ConsumerGone,
            Ending::Unreachable { error } => Verdict::Unanswered { error },

            Ending::StreamClosed {
                delivered: true,
                resume_from,
            } => Verdict::Delivered {
                mark: resume_from,
                error: None,
            },
            Ending::StreamClosed {
                delivered: false, ..
            } => Verdict::Refused {
                error: "stream closed without delivering anything".to_string(),
            },

            Ending::Failed {
                error,
                delivered: true,
                resume_from,
            } => Verdict::Delivered {
                mark: resume_from,
                error: Some(error),
            },
            Ending::Failed {
                error,
                delivered: false,
                ..
            } => Verdict::Refused { error },

            Ending::Stalled {
                error,
                delivered: true,
                resume_from,
            } => Verdict::Delivered {
                mark: resume_from,
                error: Some(error),
            },
            Ending::Stalled {
                error,
                delivered: false,
                ..
            } => Verdict::Unanswered { error },
        }
    }
}

impl Verdict {
    /// Where the next attempt resumes from, given the mark the loop already
    /// `held`.
    ///
    /// ⚠️ **An absent mark is not a mark at zero.** A session can deliver and
    /// have nothing to resume from (an unroutable transaction is dropped before
    /// the buffer), so a delivered mark *completes* the one held and never
    /// replaces it with nothing.
    ///
    /// ⚠️ **A mark is given up only to the server that refused it.** A slot
    /// past its retention, say, is not asked for twice: the next attempt starts
    /// from the live edge.
    ///
    /// ⚠️ **No floor under the mark.** A replay that breaks before the buffer
    /// drains resumes `REWIND_SLOTS` earlier each round, and the churn resets
    /// the budget, so a provider that breaks after one transaction walks
    /// `from_slot` backwards without bound.
    pub(super) fn next_resume_from(&self, held: Option<u64>) -> Option<u64> {
        match self {
            Verdict::Delivered { mark, .. } => mark.or(held),
            Verdict::Refused { .. } => None,
            Verdict::Unanswered { .. } | Verdict::ShutdownRequested | Verdict::ConsumerGone => held,
        }
    }
}
