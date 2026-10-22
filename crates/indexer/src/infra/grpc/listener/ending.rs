//! How one connection ended, what that ending is worth, and what it does to the
//! resume mark.

use tonic::Code;

use crate::infra::grpc::metrics::{GrpcListenerMetrics, MarkRefusal};

use super::log;

/// How one connection ended. Stream endings carry `resume_from` and `delivered` (whether data
/// came off the stream); what an ending is worth is [`Ending::verdict`]'s alone.
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
        /// The `Status` code, the server's or the one tonic built for our transport.
        code: Code,
        delivered: bool,
        resume_from: Option<u64>,
    },
    /// The server said nothing for [`STALL_TIMEOUT`](super::stall_clock::STALL_TIMEOUT), and this
    /// listener ended the attempt.
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
    /// The server refused the request itself: the one verdict that gives the mark up.
    Refused { error: String, reason: MarkRefusal },
    /// The server answered and gave nothing back, for a reason the request does not cause:
    /// credentials, quota, an incident, or a close with nothing sent.
    ServerFailed { error: String },
    /// Nobody answered the request: the service was unreachable, or silent.
    Unanswered { error: String },
}

impl Ending {
    /// What this ending is worth.
    /// ⚠️ A stall is no refusal: one that delivered nothing is silence, and keeps the mark.
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
            } => Verdict::ServerFailed {
                error: "stream closed without delivering anything".to_string(),
            },

            Ending::Failed {
                error,
                delivered: true,
                resume_from,
                ..
            } => Verdict::Delivered {
                mark: resume_from,
                error: Some(error),
            },
            Ending::Failed {
                error,
                code,
                delivered: false,
                ..
            } => match refusal_of(code) {
                Some(reason) => Verdict::Refused { error, reason },
                None => Verdict::ServerFailed { error },
            },

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
    /// Where the next attempt resumes from, given the mark the loop already `held`.
    /// ⚠️ No floor: a replay breaking before the buffer drains rewinds `REWIND_SLOTS` each round and
    /// the churn resets the budget, so `from_slot` can walk back without bound.
    pub(super) fn next_resume_from(&self, held: Option<u64>) -> Option<u64> {
        match self {
            // ⚠️ An absent mark is not zero (an unroutable transaction never enters the buffer).
            Verdict::Delivered { mark, .. } => mark.or(held),
            Verdict::Refused { reason, .. } => {
                if let Some(slot) = held {
                    log::mark_abandoned(slot, *reason);
                    GrpcListenerMetrics::record_mark_abandoned(*reason);
                }
                None
            }
            Verdict::ServerFailed { .. }
            | Verdict::Unanswered { .. }
            | Verdict::ShutdownRequested
            | Verdict::ConsumerGone => held,
        }
    }
}

/// Whether a code says the request itself is at fault, which alone gives the mark up.
/// ⚠️ Any other code keeps it, so a server that refuses `from_slot` under another code stops the
/// indexer at the end of the budget instead of leaving a gap.
fn refusal_of(code: Code) -> Option<MarkRefusal> {
    match code {
        Code::InvalidArgument => Some(MarkRefusal::InvalidArgument),
        Code::OutOfRange => Some(MarkRefusal::OutOfRange),
        Code::NotFound => Some(MarkRefusal::NotFound),
        _ => None,
    }
}
