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
/// All amount and reserve fields follow the **stable canonical ordering**
/// `(token_a, token_b)`, defined by sorting the two mints by their raw
/// pubkey bytes. This is the same convention used by [`crate::domain::Pool`].
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
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
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
