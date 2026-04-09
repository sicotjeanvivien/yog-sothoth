use crate::protocols::meteora::damm_v2::{detector, reserves, transfer};
use crate::protocols::meteora::{extract_signature, extract_timestamp};
use crate::types::DammV2SwapResult;
use crate::{CoreError, CoreResult};
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Parse a DAMM v2 swap from a confirmed transaction.
pub fn parse_swap(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    pool_address: &str,
) -> CoreResult<Option<DammV2SwapResult>> {
    if !detector::is_swap(tx) {
        return Ok(None);
    }

    let meta = tx
        .transaction
        .meta
        .as_ref()
        .ok_or_else(|| CoreError::ParseError {
            signature: String::new(),
            reason: "missing transaction meta".to_string(),
        })?;

    let signature = extract_signature(tx)?;
    let timestamp = extract_timestamp(tx)?;

    // Find the two transferChecked instructions that follow the DAMM v2 swap
    let (transfer_in, transfer_out, vault_a, vault_b) =
        transfer::extract_swap_transfers(meta, &signature)?;

    // Extract reserves from pre/post token balances
    let (reserve_a_before, reserve_b_before, reserve_a_after, reserve_b_after) =
        reserves::extract_reserves(tx, meta, &vault_a, &vault_b, &signature)?;

    Ok(Some(DammV2SwapResult {
        pool_address: pool_address.to_string(),
        token_in_mint: transfer_in.mint,
        token_out_mint: transfer_out.mint,
        amount_in: transfer_in.amount,
        amount_out: transfer_out.amount,
        reserve_a_before,
        reserve_b_before,
        reserve_a_after,
        reserve_b_after,
        signature,
        timestamp,
    }))
}
