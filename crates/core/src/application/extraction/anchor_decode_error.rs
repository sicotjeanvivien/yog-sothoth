/// Errors produced while extracting or decoding Anchor events from a transaction.
#[derive(Debug, thiserror::Error)]
pub enum AnchorDecodeError {
    /// Shorter than an instruction discriminator (`min` 8), or tagged but too short to hold an
    /// event discriminator after the tag (`min` 16).
    #[error("payload too short: expected at least {min} bytes, got {got}")]
    PayloadTooShort { min: usize, got: usize },

    /// Not an Anchor event emission: the data does not start with the `event_cpi` tag.
    #[error("not an Anchor event_cpi instruction: tag mismatch")]
    NotAnAnchorEvent,
}
