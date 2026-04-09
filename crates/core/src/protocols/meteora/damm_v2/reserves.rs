use crate::protocols::meteora::{extract_account_keys, find_balance};
use crate::{CoreError, CoreResult};
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionStatusMeta,
};

/// Extract pre/post reserves for the two pool vaults.
pub(super) fn extract_reserves(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    meta: &UiTransactionStatusMeta,
    vault_a_address: &str,
    vault_b_address: &str,
    signature: &str,
) -> CoreResult<(u64, u64, u64, u64)> {
    use solana_transaction_status::option_serializer::OptionSerializer;

    // Extract account keys from the message
    let account_keys = extract_account_keys(tx, signature)?;

    // Find account indices for the two vaults
    let vault_a_idx = account_keys
        .iter()
        .position(|k| k == vault_a_address)
        .ok_or_else(|| CoreError::ParseError {
            signature: signature.to_string(),
            reason: format!("vault_a not found in account keys: {vault_a_address}"),
        })? as u8;

    let vault_b_idx = account_keys
        .iter()
        .position(|k| k == vault_b_address)
        .ok_or_else(|| CoreError::ParseError {
            signature: signature.to_string(),
            reason: format!("vault_b not found in account keys: {vault_b_address}"),
        })? as u8;

    let pre = match &meta.pre_token_balances {
        OptionSerializer::Some(b) => b,
        _ => {
            return Err(CoreError::MissingField {
                signature: signature.to_string(),
                field: "preTokenBalances".to_string(),
            })
        }
    };

    let post = match &meta.post_token_balances {
        OptionSerializer::Some(b) => b,
        _ => {
            return Err(CoreError::MissingField {
                signature: signature.to_string(),
                field: "postTokenBalances".to_string(),
            })
        }
    };

    let reserve_a_before = find_balance(pre, vault_a_idx, signature)?;
    let reserve_b_before = find_balance(pre, vault_b_idx, signature)?;
    let reserve_a_after = find_balance(post, vault_a_idx, signature)?;
    let reserve_b_after = find_balance(post, vault_b_idx, signature)?;

    Ok((
        reserve_a_before,
        reserve_b_before,
        reserve_a_after,
        reserve_b_after,
    ))
}
