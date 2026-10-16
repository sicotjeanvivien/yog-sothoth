//! How long the server has said nothing that proves the stream alive.

use std::time::Duration;

use tokio::time::Instant;

/// How long the listener waits for an answer to `subscribe`, or for a block-meta (one per ~400 ms
/// slot), before ending the attempt as stalled.
/// ⚠️ A server can stop delivering and keep pinging (`rpcpool/yellowstone-grpc` #25, #175).
pub(crate) const STALL_TIMEOUT: Duration = Duration::from_secs(30);

/// The server's silence, in time spent **waiting on it**; only a block-meta resets it.
/// ⚠️ Not wall-clock time: a listener parked on a full consumer is not a silent server.
pub(super) struct StallClock {
    timeout: Duration,
    silent: Duration,
    waiting_since: Option<Instant>,
}

impl StallClock {
    pub(super) fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            silent: Duration::ZERO,
            waiting_since: None,
        }
    }

    /// The listener starts waiting on the stream.
    pub(super) fn wait_started(&mut self, at: Instant) {
        self.waiting_since = Some(at);
    }

    /// Something came off the stream; the wait that preceded it counts.
    pub(super) fn wait_ended(&mut self, at: Instant) {
        if let Some(since) = self.waiting_since.take() {
            self.silent += at.saturating_duration_since(since);
        }
    }

    /// A block-meta arrived: the stream is alive.
    pub(super) fn heard_block_meta(&mut self) {
        self.silent = Duration::ZERO;
    }

    /// How much longer the server may stay silent.
    pub(super) fn remaining(&self) -> Duration {
        self.timeout.saturating_sub(self.silent)
    }
}

#[cfg(test)]
#[path = "../tests/stall_clock_tests.rs"]
mod tests;
