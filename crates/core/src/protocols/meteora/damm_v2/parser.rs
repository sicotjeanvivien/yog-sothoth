use crate::protocols::meteora::damm_v2::{reserves, transfer};
use crate::protocols::meteora::{extract_signature, extract_timestamp};
use crate::{domain::SwapEvent, CoreError, CoreResult};
use solana_pubkey::Pubkey;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Parse a DAMM v2 swap from a confirmed transaction.
pub(super) fn parse_swap(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    pool_address: Pubkey,
    program_id_str: &str,
) -> CoreResult<SwapEvent> {
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
        transfer::extract_swap_transfers(meta, &signature, program_id_str)?;

    // Extract reserves from pre/post token balances
    let (reserve_a_before, reserve_b_before, reserve_a_after, reserve_b_after) =
        reserves::extract_reserves(tx, meta, &vault_a, &vault_b, &signature)?;

    Ok(SwapEvent {
        pool_address: pool_address,
        token_in_mint: transfer_in.mint,
        token_out_mint: transfer_out.mint,
        amount_in: transfer_in.amount,
        amount_out: transfer_out.amount,
        reserve_a_before,
        reserve_b_before,
        reserve_a_after,
        reserve_b_after,
        fee_bps: None,
        fee_amount: None,
        signature,
        timestamp,
    })
}
