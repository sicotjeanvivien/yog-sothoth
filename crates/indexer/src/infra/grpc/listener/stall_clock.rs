//! How long the server has said nothing that proves the stream alive.

use std::time::Duration;

use tokio::time::Instant;

/// How long the listener waits for an answer to `subscribe`, or for a
/// block-meta on the stream, before ending the attempt as stalled.
///
/// ⚠️ A server can stop delivering and keep the connection open — HTTP/2
/// keep-alive and Yellowstone pings both carry on, the pings coming from a
/// task of their own (`rpcpool/yellowstone-grpc` #25, #175). Without this
/// bound the listener would wait for ever.
///
/// The block-meta is the signal because one comes every slot (~400 ms)
/// whatever the market does. 30 s is Alchemy's alerting threshold; not
/// measured — the largest block-meta gap on a real stream should replace it.
pub(crate) const STALL_TIMEOUT: Duration = Duration::from_secs(30);

/// The silence of the server, measured in time spent **waiting on it**.
///
/// ⚠️ Not wall-clock time: between two waits the listener may sit in
/// `StreamSession::handle`, parked on a full consumer, and a slow database is
/// not a silent server. Only the span from [`Self::wait_started`] to
/// [`Self::wait_ended`] counts, and only a block-meta resets it.
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
