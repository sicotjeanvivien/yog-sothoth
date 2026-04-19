use crate::domain::LiquidityEventKind;
use crate::protocols::meteora::damm_v2::{reserves, transfer};
use crate::protocols::meteora::{extract_signature, extract_timestamp};
use crate::{
    domain::{LiquidityEvent, SwapEvent},
    CoreError, CoreResult,
};
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

use super::pool;

/// Parse a DAMM v2 swap from a confirmed transaction.
pub(super) fn parse_swap(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
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

    // Discover the pool from the transaction instructions
    let pool_address = pool::extract_pool_address(tx, program_id_str, &signature)?;

    // Find the two transferChecked instructions that follow the DAMM v2 swap
    let (transfer_in, transfer_out, vault_a, vault_b) =
        transfer::extract_swap_transfers(tx, meta, &signature, program_id_str)?;

    // Extract reserves from pre/post token balances
    let (reserve_a_before, reserve_b_before, reserve_a_after, reserve_b_after) =
        reserves::extract_reserves(tx, meta, &vault_a, &vault_b, &signature)?;

    Ok(SwapEvent {
        pool_address,
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

pub(super) fn parse_liquidity(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    program_id_str: &str,
    liquidity_kind: LiquidityEventKind,
) -> CoreResult<LiquidityEvent> {
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

    // Discover the pool from the transaction instructions
    let pool_address = pool::extract_pool_address(tx, program_id_str, &signature)?;

    let (transfer_a, transfer_b) =
        transfer::extract_liquidity_transfers(tx, meta, &signature, program_id_str)?;

    Ok(LiquidityEvent {
        pool_address,
        liquidity_event_kind: liquidity_kind,
        amount_a: transfer_a.amount,
        amount_b: transfer_b.amount,
        signature,
        timestamp,
    })
}
