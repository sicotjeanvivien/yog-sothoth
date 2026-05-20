use crate::solana_types::{EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction};
use crate::{CoreError, CoreResult};
use chrono::{DateTime, Utc};

/// Extract the first transaction signature.
pub(crate) fn extract_signature(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> CoreResult<String> {
    match &tx.transaction.transaction {
        EncodedTransaction::Json(ui_tx) => {
            ui_tx
                .signatures
                .first()
                .cloned()
                .ok_or_else(|| CoreError::MissingField {
                    signature: String::new(),
                    field: "signatures".to_string(),
                })
        }
        _ => Err(CoreError::ParseError {
            signature: String::new(),
            reason: "unexpected transaction encoding".to_string(),
        }),
    }
}

/// Extract the block timestamp as UTC.
pub(crate) fn extract_timestamp(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> CoreResult<DateTime<Utc>> {
    let block_time = tx.block_time.ok_or_else(|| CoreError::MissingField {
        signature: String::new(),
        field: "blockTime".to_string(),
    })?;

    DateTime::from_timestamp(block_time, 0).ok_or_else(|| CoreError::ParseError {
        signature: String::new(),
        reason: format!("invalid timestamp: {block_time}"),
    })
}
