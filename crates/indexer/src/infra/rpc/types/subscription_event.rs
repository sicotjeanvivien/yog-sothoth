use solana_pubkey::Pubkey;
use yog_core::domain::Protocol;

/// Lifecycle events a `SubscriptionWorker` broadcasts to the listener: informational only, the
/// worker owns its reconnection.
#[derive(Debug, Clone)]
pub(crate) enum SubscriptionEvent {
    /// The worker is streaming logs, after every successful subscribe.
    Subscribed { protocol: Protocol, mention: Pubkey },

    /// The subscription stream closed (provider-side idle timeout, silent
    /// reconnect, etc.). The worker will attempt to resubscribe.
    StreamClosed {
        protocol: Protocol,
        mention: Pubkey,
        attempt: u32,
    },

    /// A subscribe attempt failed; the worker keeps retrying until its budget is spent.
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
