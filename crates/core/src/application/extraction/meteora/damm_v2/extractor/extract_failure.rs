use crate::application::extraction::{AnchorDecodeError, DISCRIMINATOR_LEN};

/// A failed extraction attempt and its reason.
#[derive(Debug, thiserror::Error)]
pub enum ExtractFailure {
    /// Neither an ordinary instruction nor a decodable event_cpi payload.
    #[error("anchor event_cpi decode failed: {source}")]
    AnchorDecode {
        #[source]
        source: AnchorDecodeError,
    },

    /// Known discriminator, but borsh failed: usually schema drift in our mirror.
    #[error("borsh deserialization of {event_name} failed: {reason}")]
    Borsh {
        event_name: &'static str,
        discriminator: [u8; DISCRIMINATOR_LEN],
        reason: String,
    },

    /// More inner instructions addressed to cp-amm than an `event_index` can number.
    /// ⚠️ Dropped, never truncated: a wrong index is a duplicate or a lost row.
    #[error("event index {index} exceeds u16")]
    EventIndexOverflow { index: usize },
}
