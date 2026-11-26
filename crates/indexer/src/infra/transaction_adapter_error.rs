/// Why a transaction adapter could not turn a source's transaction into an `OnChainTransaction`.
#[derive(Debug, thiserror::Error)]
pub(crate) enum TransactionAdapterError {
    #[error("missing field `{field}` in transaction {signature}")]
    MissingField { signature: String, field: String },
    #[error("failed to parse transaction {signature}: {reason}")]
    Parse { signature: String, reason: String },
}
