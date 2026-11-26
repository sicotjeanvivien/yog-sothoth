use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::domain::{Protocol, Severity};

/// The last item of a page in the feed ordering, `(triggered_at, id)` descending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalCursor {
    pub triggered_at: DateTime<Utc>,
    pub id: i64,
}

/// What a page of the feed keeps: each set field narrows it, and they combine with AND.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignalFilter {
    pub severity: Option<Severity>,
    pub protocol: Option<Protocol>,
    pub pool: Option<Pubkey>,
}
