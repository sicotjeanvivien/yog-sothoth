use crate::domain::{LiquidityEventKind, Protocol};
use crate::protocols::meteora::damm_v2::{reserves, transfer};
use crate::protocols::meteora::{extract_signature, extract_timestamp};
use crate::{
    domain::{LiquidityEvent, SwapEvent},
    CoreError, CoreResult,
};
use solana_pubkey::Pubkey;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

use super::pool;

/// Parse a DAMM v2 swap from a confirmed transaction.
///
/// Extracts on-chain facts only — no derived metrics.
///
/// # How token_a / token_b are determined
///
/// Mints are sorted by **raw pubkey bytes** (not base58 alphabetical order)
/// to ensure stability across swap directions: the same pool always yields
/// the same `(token_a, token_b)` regardless of whether the observed swap
/// is A→B or B→A.
///
/// This differs from the Meteora SDK's canonical ordering — adjust at query
/// time if alignment is needed.
///
/// # Errors
///
/// Returns `ParseError` if the transaction does not contain a DAMM v2 swap
/// instruction, or if the expected `transferChecked` CPIs are not found.
pub(super) fn parse_swap(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    protocol: Protocol,
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

    let (token_a_mint, token_b_mint) = sort_mints(transfer_in.mint, transfer_out.mint);

    // Extract reserves from pre/post token balances
    let (reserve_in_before, reserve_out_before, reserve_in_after, reserve_out_after) =
        reserves::extract_reserves(tx, meta, &vault_a, &vault_b, &signature)?;

    // TODO(phase 2): extract fee_bps and fee_amount from DAMM v2 swap event logs
    Ok(SwapEvent {
        pool_address,
        protocol,
        token_a_mint,
        token_b_mint,
        token_in_mint: transfer_in.mint,
        token_out_mint: transfer_out.mint,
        amount_in: transfer_in.amount,
        amount_out: transfer_out.amount,
        reserve_in_before,
        reserve_out_before,
        reserve_in_after,
        reserve_out_after,
        fee_bps: None,
        fee_amount: None,
        signature,
        timestamp,
    })
}

/// Parse a DAMM v2 add/remove liquidity event from a confirmed transaction.
///
/// Extracts on-chain facts only — no derived metrics.
///
/// Mints are sorted by raw pubkey bytes — see [`parse_swap`] for rationale.
///
/// # Errors
///
/// Returns `ParseError` if the transaction does not contain a DAMM v2
/// liquidity instruction, or if the expected `transferChecked` CPIs
/// are not found.
pub(super) fn parse_liquidity(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    protocol: Protocol,
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

    // Align mints and amounts together by raw pubkey bytes, so `amount_a`
    // always refers to `token_a_mint` regardless of transfer ordering.
    let (token_a_mint, token_b_mint, amount_a, amount_b) = if transfer_a.mint <= transfer_b.mint {
        (
            transfer_a.mint,
            transfer_b.mint,
            transfer_a.amount,
            transfer_b.amount,
        )
    } else {
        (
            transfer_b.mint,
            transfer_a.mint,
            transfer_b.amount,
            transfer_a.amount,
        )
    };

    Ok(LiquidityEvent {
        pool_address,
        protocol,
        token_a_mint,
        token_b_mint,
        liquidity_event_kind: liquidity_kind,
        amount_a,
        amount_b,
        signature,
        timestamp,
    })
}

/// Stabilise mint pair ordering by sorting on raw pubkey bytes.
///
/// Uses the default `Ord` impl of `Pubkey`, which compares the underlying
/// `[u8; 32]` byte-wise — this is not the same as base58 alphabetical order.
/// The invariant that matters is **determinism**: the same input pair
/// always yields the same output order.
fn sort_mints(m1: Pubkey, m2: Pubkey) -> (Pubkey, Pubkey) {
    if m1 <= m2 {
        (m1, m2)
    } else {
        (m2, m1)
    }
}
