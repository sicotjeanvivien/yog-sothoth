use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::domain::TradeDirection;

/// Raw swap event extracted from an on-chain Anchor event.
///
/// Captures only on-chain facts — no derived analytics (price, slippage,
/// imbalance). Those are computed at query time from the reserves recorded
/// here.
///
/// # Mint and amount conventions
///
/// All amount and reserve fields are in the pool's **`(token_a, token_b)`
/// order as the program defines it** — the designation read off the on-chain
/// `Pool` account and stored as-is on [`crate::domain::Pool`].
///
/// **It is not a sort.** cp-amm does not order the pair by pubkey bytes, and
/// nothing downstream re-orders it: measured on the local index, roughly a
/// third of pools have `token_a_mint > token_b_mint`. What the convention
/// guarantees is *internal consistency* — `amount_a` / `reserve_a_after`, the
/// `token_a_mint` column, and the direction of `sqrt_price_to_price_a_in_b` all
/// mean the same side of the same pool, and that side is stable over the pool's
/// life.
///
/// What it does **not** give you is any relation between the two mints, so this
/// is not a canonical pair key. Deduplicating a pair across pools, or joining a
/// DAMM v2 pool to a DLMM one on "the same pair", needs a key built explicitly —
/// never `token_a_mint` on the assumption that it is already the smaller of the
/// two. The wrong answer there is silent and lands on a third of the table.
///
/// One trap inside the remedy: the mints are stored as base58 `TEXT`, so a SQL
/// `LEAST(a, b)` / `GREATEST(a, b)` orders them *lexicographically in base58*,
/// while a Rust-side key from `Pubkey::min` / `max` orders them by **raw
/// bytes**. The two disagree on some pairs. Either is a valid canonical key on
/// its own; mixing them silently splits a pair in half, so a key that crosses
/// the boundary has to pick one convention and say which.
///
/// To recover the trader's perspective:
/// - `trade_direction == AtoB` → trader sent `amount_a`, received `amount_b`
/// - `trade_direction == BtoA` → trader sent `amount_b`, received `amount_a`
///
/// # Reserves
///
/// `reserve_a_after` and `reserve_b_after` reflect the pool's accounting
/// reserves (`pool.token_a_amount` / `pool.token_b_amount`) **immediately
/// after the swap is applied**. They do NOT include accrued protocol fees,
/// which are tracked separately in the on-chain `Pool` state — so they may
/// differ from the raw vault balances. Use them for AMM-state analytics
/// (price, slippage, k invariant), not for vault accounting.
///
/// # Fees
///
/// The four fee components correspond directly to the on-chain `SwapResult2`:
/// - [`claiming_fee`] — claimable by LPs via `claim_position_fee`
/// - [`protocol_fee`] — collected by Meteora
/// - [`compounding_fee`] — re-injected into the pool's liquidity (compounding
///   pools only; otherwise zero)
/// - [`referral_fee`] — paid out to the referrer (only if `has_referral`
///   is set on the swap; otherwise zero)
///
/// The total fee charged on the swap is the sum of all four. It is borne
/// by token A or token B depending on `fee_token_is_a` (which itself is a
/// function of the pool's `collect_fee_mode` and the swap's direction).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2SwapEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain — see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub trade_direction: TradeDirection,
    pub amount_a: u64,
    pub amount_b: u64,
    pub reserve_a_after: u64,
    pub reserve_b_after: u64,
    pub next_sqrt_price: u128,
    pub claiming_fee: u64,
    pub protocol_fee: u64,
    pub compounding_fee: u64,
    pub referral_fee: u64,
    pub fee_token_is_a: bool,
}

impl MeteoraDammV2SwapEvent {
    /// Convenience: total fee charged on this swap, in the unit of whichever
    /// token bore the fee (see `fee_token_is_a`).
    pub fn fee_total(&self) -> u64 {
        self.claiming_fee
            .saturating_add(self.protocol_fee)
            .saturating_add(self.compounding_fee)
            .saturating_add(self.referral_fee)
    }
}
