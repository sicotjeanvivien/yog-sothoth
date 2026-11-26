//! What every protocol extractor returns: domain events, plus what could not be processed.

use solana_pubkey::Pubkey;

use super::ExtractionFailure;
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
