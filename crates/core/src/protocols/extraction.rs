//! Common types for the multi-protocol event extraction pipeline.
//!
//! Every protocol implementation of [`crate::protocols::PoolIndexer`]
//! returns an [`ExtractionOutcome`] carrying domain events alongside
//! diagnostic information about events that could not be processed.

use solana_pubkey::Pubkey;
use thiserror::Error;

use crate::domain::{DomainEvent, Protocol};

/// Result of extracting domain events from a single transaction.
///
/// Always well-formed — extraction never fails as a whole. Per-event
/// problems are reported through `unknown` and `failures` so that callers
/// can emit metrics with appropriate cardinality.
#[derive(Debug, Default)]
pub struct ExtractionOutcome {
    /// Successfully extracted, decoded, and translated domain events.
    /// Order reflects the order of appearance in the transaction.
    pub events: Vec<DomainEvent>,

    /// Anchor events whose discriminator is unknown to the protocol's
    /// extractor. Most often correspond to events from rings we haven't
    /// implemented yet.
    pub unknown: Vec<UnknownEventInfo>,

    /// Events that targeted the protocol but failed somewhere in the
    /// pipeline (decoding, deserialization, or translation).
    pub failures: Vec<ExtractionFailure>,
}

/// Description of an unrecognized Anchor event encountered during extraction.
#[derive(Debug, Clone, Copy)]
pub struct UnknownEventInfo {
    pub protocol: Protocol,
    pub discriminator: [u8; 8],
}

/// A failed extraction attempt, paired with the reason why.
#[derive(Debug, Error)]
pub enum ExtractionFailure {
    /// The Anchor event_cpi payload could not be decoded (wrong tag,
    /// payload too short, etc.).
    #[error("anchor decoding: {0}")]
    AnchorDecode(String),

    /// Discriminator matched a known event but the borsh payload could
    /// not be deserialized — usually a schema drift between the program
    /// and our wire mirrors.
    #[error("borsh deserialization of {event_name}: {reason}")]
    Borsh {
        event_name: &'static str,
        reason: String,
    },

    /// Wire event was decoded successfully but could not be translated
    /// into a domain event — for example, the transferChecked context
    /// expected by the translator was missing.
    #[error("translation of {event_name}: {reason}")]
    Translation {
        event_name: &'static str,
        reason: String,
    },
}

/// Companion to [`UnknownEventInfo::discriminator`], for callers that want
/// a stable hex representation suitable for metrics labels.
pub fn discriminator_hex(disc: &[u8; 8]) -> String {
    let mut s = String::with_capacity(16);
    for b in disc {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

impl UnknownEventInfo {
    /// Pubkey form of the program emitting the event — handy for callers
    /// that want to label metrics with the program ID rather than the
    /// protocol enum.
    pub fn program_id(&self) -> Pubkey {
        self.protocol.program_id()
    }
}