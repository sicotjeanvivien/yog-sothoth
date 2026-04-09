use crate::{CoreError, CoreResult};
use chrono::{DateTime, Utc};
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta,
    EncodedTransaction,
    UiMessage,
    UiTransactionTokenBalance,
};

/// Extract the first transaction signature.
pub(crate) fn extract_signature(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> CoreResult<String> {
    match &tx.transaction.transaction {
        EncodedTransaction::Json(ui_tx) => ui_tx
            .signatures
            .first()
            .cloned()
            .ok_or_else(|| CoreError::MissingField {
                signature: String::new(),
                field: "signatures".to_string(),
            }),
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

/// Extract the ordered list of account public keys from the transaction message.
pub(crate) fn extract_account_keys(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    signature: &str,
) -> CoreResult<Vec<String>> {
    let ui_tx = match &tx.transaction.transaction {
        EncodedTransaction::Json(ui_tx) => ui_tx,
        _ => return Err(CoreError::ParseError {
            signature: signature.to_string(),
            reason: "unexpected transaction encoding".to_string(),
        }),
    };

    let keys = match &ui_tx.message {
        UiMessage::Parsed(msg) => msg.account_keys.iter().map(|k| k.pubkey.clone()).collect(),
        UiMessage::Raw(msg) => msg.account_keys.clone(),
    };

    Ok(keys)
}

/// Find the token balance amount for a given account index.
pub(crate) fn find_balance(
    balances: &[UiTransactionTokenBalance],
    account_index: u8,
    signature: &str,
) -> CoreResult<u64> {
    balances
        .iter()
        .find(|b| b.account_index == account_index)
        .and_then(|b| b.ui_token_amount.amount.parse::<u64>().ok())
        .ok_or_else(|| CoreError::ParseError {
            signature: signature.to_string(),
            reason: format!("no token balance found for account index {account_index}"),
        })
}