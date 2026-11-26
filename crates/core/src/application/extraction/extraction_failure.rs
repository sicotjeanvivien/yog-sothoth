use thiserror::Error;

/// A failed extraction attempt, paired with the reason why.
#[derive(Debug, Error)]
pub enum ExtractionFailure {
    /// Neither an ordinary instruction nor a decodable event_cpi payload.
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
