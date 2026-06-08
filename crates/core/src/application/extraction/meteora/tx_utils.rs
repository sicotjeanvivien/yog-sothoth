use std::str::FromStr;

use crate::solana_types::{EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction};
use crate::{CoreError, CoreResult};
use chrono::{DateTime, Utc};
use solana_signature::Signature;

/// Extract the first transaction signature.
pub(crate) fn extract_signature(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> CoreResult<Signature> {
    match &tx.transaction.transaction {
        EncodedTransaction::Json(ui_tx) => {
            let sig_str = ui_tx
                .signatures
                .first()
                .ok_or_else(|| CoreError::MissingField {
                    signature: String::new(),
                    field: "signatures".to_string(),
                })?;

            Signature::from_str(sig_str).map_err(|e| CoreError::ParseError {
                signature: String::new(),
                reason: format!("invalid signature {sig_str}: {e}"),
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
