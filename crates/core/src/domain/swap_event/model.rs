use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// Raw swap event parsed from a DAMM v2 transaction.
///
/// Contains only on-chain data — no derived metrics.
/// Metrics (price, price impact, imbalance, fees collected) are computed
/// by the indexer from this struct and written separately to `pool_metrics`.
///
/// Raw swap event parsed from a DAMM v2 transaction.
///
/// Contains only on-chain data — no derived metrics.
/// Metrics (price, price impact, imbalance, fees collected) are computed
/// by the indexer from this struct and written separately to `pool_metrics`.
///
/// # Amounts and reserves
///
/// All amounts are in native units (no decimal scaling).
///
/// - `amount_in` / `amount_out` and `reserve_in_*` / `reserve_out_*` follow
///   the **direction of the swap** — `in` is what the trader sent, `out` is
///   what they received.
/// - `token_a_mint` / `token_b_mint` follow the **stable pool convention**
///   (sorted by raw pubkey bytes, see [`crate::domain::Pool`]).
///
/// To align reserves with token A/B, compare `token_in_mint` with `token_a_mint`
/// at query time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEvent {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Protocol that emitted this swap (used for routing and filtering).
    pub protocol: Protocol,

    /// Mint of token A in **stable order** — see struct-level docs for convention.
    pub token_a_mint: Pubkey,

    /// Mint of token B in **stable order** — see struct-level docs for convention.
    pub token_b_mint: Pubkey,

    /// Mint address of the token sold by the trader (token flowing in).
    pub token_in_mint: Pubkey,

    /// Mint address of the token received by the trader (token flowing out).
    pub token_out_mint: Pubkey,

    /// Amount of `token_in` consumed, in native units.
    pub amount_in: u64,

    /// Amount of `token_out` received, in native units.
    pub amount_out: u64,

    /// Reserve of the token the trader sent (`token_in`), immediately before the swap.
    pub reserve_in_before: u64,

    /// Reserve of the token the trader received (`token_out`), immediately before the swap.
    pub reserve_out_before: u64,

    /// Reserve of `token_in`, immediately after the swap.
    pub reserve_in_after: u64,

    /// Reserve of `token_out`, immediately after the swap.
    pub reserve_out_after: u64,

    /// Protocol fee rate in basis points (1 bps = 0.01 %).
    /// `None` when the protocol does not expose fee data in the transaction.
    pub fee_bps: Option<u32>,

    /// Absolute fee amount collected on this swap, in native units of `token_in`.
    /// `None` if not directly exposed by the protocol — the indexer will
    /// derive it as `amount_in * fee_bps / 10_000` before writing to `pool_metrics`.
    pub fee_amount: Option<u64>,

    /// Transaction signature, base58-encoded.
    pub signature: String,

    /// Block timestamp at which the transaction was confirmed.
    pub timestamp: DateTime<Utc>,
}
