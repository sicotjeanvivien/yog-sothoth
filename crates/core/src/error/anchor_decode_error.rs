/// Errors produced while extracting or decoding Anchor events from a transaction.
#[derive(Debug, thiserror::Error)]
pub enum AnchorDecodeError {
    /// The instruction's `data` field could not be decoded as base58.
    #[error("invalid base58 in instruction data: {0}")]
    InvalidBase58(String),

    /// The instruction data is shorter than the minimum required to
    /// contain the Anchor event tag plus a discriminator.
    #[error("payload too short: expected at least {min} bytes, got {got}")]
    PayloadTooShort { min: usize, got: usize },

    /// The first 8 bytes do not match the Anchor `event_cpi` tag — this
    /// inner instruction is not an Anchor event emission.
    #[error("not an Anchor event_cpi instruction: tag mismatch")]
    NotAnAnchorEvent,
}
