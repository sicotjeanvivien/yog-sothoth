//! How one connection ended, and what that does to the resume mark.

/// How one connection ended.
///
/// The stream endings carry two facts the listener needs on each:
/// `resume_from`, where to pick up (see `StreamSession::resume_from`), and
/// `delivered`, whether any data came off the stream — what separates the
/// churn of a long-lived connection from a server that accepts and closes at
/// once. [`Attempt::Unreachable`] carries neither: nothing reached the service.
pub(super) enum Attempt {
    ShutdownRequested,
    DownstreamClosed,
    /// The attempt never got an answer from the service.
    ///
    /// ⚠️ Not a variety of `Failed`: no `from_slot` of ours was accepted or
    /// refused, so it is no verdict on the mark. Three sites produce it — the
    /// dial, `subscribe` failing on **our** transport (a link cut after the TCP
    /// handshake; see `reached_the_service`), and the outbound half, which
    /// nothing reaches today.
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
    /// ⚠️ `Failed`'s budget rule, not its mark rule: silence is not a refusal,
    /// so a stall that delivered nothing keeps the mark (the budget bounds the
    /// retries). A stall that did deliver is churn and restarts the budget — a
    /// provider that replays and then goes quiet is retried for ever, which
    /// `stalls_total` rising steadily shows.
    ///
    /// [`STALL_TIMEOUT`]: super::stall_clock::STALL_TIMEOUT
    Stalled {
        error: String,
        delivered: bool,
        resume_from: Option<u64>,
    },
}

impl Attempt {
    /// Where the next attempt resumes from, given the mark the loop already
    /// `held` — the whole resume rule, in one expression.
    ///
    /// ⚠️ **An absent mark is not a mark at zero.** A session can deliver and
    /// have nothing to resume from (an unroutable transaction is dropped before
    /// the buffer), so a delivered attempt's mark *completes* the one held and
    /// never replaces it with nothing.
    ///
    /// ⚠️ **A mark is given up only to the server that refused it.** A
    /// `from_slot` the server had and gave nothing back for — a slot past its
    /// retention, say — is not asked for twice: the next attempt starts from
    /// the live edge. An ending that is no verdict (`Unreachable`, a stall)
    /// keeps the mark.
    ///
    /// ⚠️ **No floor under the mark.** A replay that breaks before the buffer
    /// drains resumes `REWIND_SLOTS` earlier each round, and the churn resets
    /// the budget, so a provider that breaks after one transaction walks
    /// `from_slot` backwards without bound.
    pub(super) fn next_resume_from(&self, held: Option<u64>) -> Option<u64> {
        match self {
            Attempt::StreamClosed {
                delivered: true,
                resume_from,
            }
            | Attempt::Failed {
                delivered: true,
                resume_from,
                ..
            }
            | Attempt::Stalled {
                delivered: true,
                resume_from,
                ..
            } => (*resume_from).or(held),

            Attempt::StreamClosed {
                delivered: false, ..
            }
            | Attempt::Failed {
                delivered: false, ..
            } => None,

            // Nothing was asked of anyone, so nothing was refused.
            Attempt::Unreachable { .. } => held,

            // Asked, and met with silence — which is not a refusal.
            Attempt::Stalled {
                delivered: false, ..
            } => held,

            // The loop returns on both, so this answer is never read.
            Attempt::ShutdownRequested | Attempt::DownstreamClosed => held,
        }
    }
}
