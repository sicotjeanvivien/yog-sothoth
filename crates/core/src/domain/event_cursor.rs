//! What a page of on-chain events is read over: every protocol, one protocol, or one pool,
//! and the key of its last event.

use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::domain::{Protocol, Window};

/// Which events a list reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventScope {
    All,
    Protocol(Protocol),
    Pool(Pubkey),
}

/// The key of the last event of a page — its instant, then its signature and its index in the
/// transaction — and the list it was read from: its scope and the start of its window, fixed at
/// its first page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventCursor {
    pub scope: EventScope,
    pub from: DateTime<Utc>,
    pub at: DateTime<Utc>,
    pub signature: Signature,
    pub event_index: u16,
}

impl EventCursor {
    /// The start of the window a page reads from: its cursor's, fixed at the first page, else
    /// `window` back from `now`.
    pub fn window_start(
        cursor: Option<&Self>,
        window: Window,
        now: DateTime<Utc>,
    ) -> DateTime<Utc> {
        cursor.map_or_else(|| now - window.duration(), |c| c.from)
    }
}

#[cfg(test)]
#[path = "event_cursor_tests.rs"]
mod tests;
