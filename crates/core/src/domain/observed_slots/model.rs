//! What one ingestion session watched: from its first to its last slot, each with the chain's
//! time. A range nobody watched is never written: it is the complement of these, read in SQL.

use chrono::{DateTime, Utc};

use crate::domain::Protocol;

/// The slots one ingestion session watched for one protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedSlots {
    pub protocol: Protocol,
    /// When the session started: the key of its row.
    pub session_started_at: DateTime<Utc>,
    pub first_slot: u64,
    pub first_slot_at: DateTime<Utc>,
    pub last_slot: u64,
    pub last_slot_at: DateTime<Utc>,
}
