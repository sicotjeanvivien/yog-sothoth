//! What every protocol extractor returns: domain events, plus what could not be processed.

use solana_pubkey::Pubkey;
use thiserror::Error;

use crate::domain::{DomainEvent, Protocol};

/// Result of extracting domain events from one transaction; never fails as a whole.
#[derive(Debug, Default)]
pub struct ExtractionOutcome {
    /// Translated domain events, in transaction order.
    pub events: Vec<DomainEvent>,

    /// Anchor events whose discriminator the extractor does not know.
    pub unknown: Vec<UnknownEventInfo>,

    /// Events of the protocol that failed to decode, deserialize or translate.
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
    /// Not a valid Anchor event_cpi payload.
    #[error("anchor decoding: {0}")]
    AnchorDecode(String),

    /// Known discriminator, but borsh failed: usually schema drift in our mirror.
    #[error("borsh deserialization of {event_name}: {reason}")]
    Borsh {
        event_name: &'static str,
        reason: String,
    },

    /// Decoded, but not translatable (e.g. an unknown enum value).
    #[error("translation of {event_name}: {reason}")]
    Translation {
        event_name: &'static str,
        reason: String,
    },

    /// Too many payloads for an `event_index`: a *valid* event was dropped,
    /// rather than stored under a truncated index.
    #[error("event index {index} exceeds the u16 an event_index can hold")]
    EventIndexOverflow { index: usize },
}

/// Stable hex form of a discriminator, for metric labels.
pub fn discriminator_hex(disc: &[u8; 8]) -> String {
    let mut s = String::with_capacity(16);
    for b in disc {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

impl UnknownEventInfo {
    /// Program emitting the event.
    pub fn program_id(&self) -> Pubkey {
        self.protocol.program_id()
    }
}
