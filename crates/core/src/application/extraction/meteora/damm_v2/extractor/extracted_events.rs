use crate::application::extraction::DISCRIMINATOR_LEN;

use super::super::events::DammV2WireEvent;
use super::ExtractFailure;

/// Outcome of extracting wire events from a transaction; never fails as a whole.
#[derive(Debug, Default)]
pub struct ExtractedEvents {
    /// Recognized events, in transaction order.
    pub events: Vec<IndexedWireEvent>,

    /// Payloads whose discriminator matches no known event: a metric, not an error.
    pub unknown: Vec<UnknownEvent>,

    /// Payloads addressed to cp-amm that failed to decode.
    pub failures: Vec<ExtractFailure>,
}

/// A recognized wire event and its position in the transaction.
/// ⚠️ `event_index` counts every inner instruction to cp-amm, `event_cpi` or not: that keeps
/// it stable across releases ([`crate::domain::EventPosition`]).
#[derive(Debug)]
pub struct IndexedWireEvent {
    pub event_index: u16,
    pub event: DammV2WireEvent,
}

/// A self-CPI payload whose discriminator matches no known event; the
/// discriminator labels the caller's metric.
#[derive(Debug, Clone, Copy)]
pub struct UnknownEvent {
    pub discriminator: [u8; DISCRIMINATOR_LEN],
}
