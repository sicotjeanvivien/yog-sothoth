//! What a page of on-chain events is read over: every protocol, one protocol, or one pool,
//! and the key of its last event.

use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::domain::Protocol;

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
