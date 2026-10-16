/// Errors produced while extracting or decoding Anchor events from a transaction.
#[derive(Debug, thiserror::Error)]
pub enum AnchorDecodeError {
    /// The instruction's `data` field could not be decoded as base58.
    #[error("invalid base58 in instruction data: {0}")]
    InvalidBase58(String),

    /// Shorter than the Anchor event tag plus a discriminator.
    #[error("payload too short: expected at least {min} bytes, got {got}")]
    PayloadTooShort { min: usize, got: usize },

    /// Not an Anchor event emission: the first 8 bytes are not the `event_cpi` tag.
    #[error("not an Anchor event_cpi instruction: tag mismatch")]
    NotAnAnchorEvent,
}
