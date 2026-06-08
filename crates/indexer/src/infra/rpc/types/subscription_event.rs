use solana_pubkey::Pubkey;
use yog_core::domain::Protocol;

/// Lifecycle events emitted by a `SubscriptionWorker` towards the listener.
///
/// The events are intentionally informational. The listener consumes them to
/// track worker health, log activity, and decide when to escalate to the
/// Daemon (when no worker is alive anymore). Workers handle their own
/// reconnection strategy — the listener does not reshape their behaviour.
///
/// Broadcast-friendly: cheap to clone, small payloads, no references.
#[derive(Debug, Clone)]
pub(crate) enum SubscriptionEvent {
    /// The worker has an active subscription streaming logs.
    /// Emitted on first successful subscribe and on every successful resubscribe.
    Subscribed { protocol: Protocol, mention: Pubkey },

    /// The subscription stream closed (provider-side idle timeout, silent
    /// reconnect, etc.). The worker will attempt to resubscribe.
    StreamClosed {
        protocol: Protocol,
        mention: Pubkey,
        attempt: u32,
    },

    /// A subscribe attempt failed. Purely informational — does not signal
    /// giving up. The worker will continue retrying with backoff until its
    /// own budget is exhausted.
    RetryFailed {
        protocol: Protocol,
        mention: Pubkey,
        attempt: u32,
        error: String,
    },

    /// The worker has exhausted its retry budget and is terminating.
    /// The listener treats this as a hard signal: one worker down.
    GivingUp {
        protocol: Protocol,
        mention: Pubkey,
        last_error: String,
    },

    /// The worker terminated cleanly after a cooperative shutdown signal.
    ShutdownCompleted { protocol: Protocol, mention: Pubkey },
}
