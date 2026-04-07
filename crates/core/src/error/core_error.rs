use thiserror::Error;

/// All errors that can occur within yog-core.
#[derive(Debug, Error)]
pub enum CoreError {
    /// The transaction does not belong to a known protocol.
    #[error("unknown program id: {0}")]
    UnknownProgram(String),

    /// The transaction was recognized but could not be parsed.
    #[error("failed to parse transaction {signature}: {reason}")]
    ParseError { signature: String, reason: String },

    /// The account data does not match the expected layout.
    #[error("invalid account data for pool {address}: {reason}")]
    InvalidAccountData { address: String, reason: String },

    /// A required field is missing from the transaction.
    #[error("missing field `{field}` in transaction {signature}")]
    MissingField { signature: String, field: String },

    /// Arithmetic overflow during AMM computation.
    #[error("arithmetic overflow in {context}")]
    ArithmeticOverflow { context: String },

    /// The program is known but the instruction is not handled.
    #[error("unsupported instruction in transaction {signature}")]
    UnsupportedInstruction { signature: String },
}
