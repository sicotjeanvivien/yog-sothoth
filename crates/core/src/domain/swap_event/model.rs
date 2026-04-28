use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::domain::{Protocol, TradeDirection};

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
pub struct SwapEvent {
    // ── Identification ──────────────────────────────────────────────────────
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Protocol that emitted this swap.
    pub protocol: Protocol,

    /// Transaction signature, base58-encoded.
    pub signature: String,

    /// Block timestamp at which the transaction was confirmed.
    pub timestamp: DateTime<Utc>,

    // ── Pool tokens (canonical order) ───────────────────────────────────────
    /// Mint of token A in the canonical ordering — see struct-level docs.
    pub token_a_mint: Pubkey,

    /// Mint of token B in the canonical ordering — see struct-level docs.
    pub token_b_mint: Pubkey,

    // ── Trade direction and amounts ─────────────────────────────────────────
    /// Direction of the swap relative to the canonical mint ordering.
    pub trade_direction: TradeDirection,

    /// Amount that moved on the token A side (in or out depending on direction).
    pub amount_a: u64,

    /// Amount that moved on the token B side (in or out depending on direction).
    pub amount_b: u64,

    // ── Post-swap pool state (canonical order) ──────────────────────────────
    /// Pool's accounting reserve of token A immediately after the swap.
    pub reserve_a_after: u64,

    /// Pool's accounting reserve of token B immediately after the swap.
    pub reserve_b_after: u64,

    /// Pool's `sqrt_price` after the swap, as a Q64.64 fixed-point integer.
    /// Useful for high-precision price calculations.
    pub next_sqrt_price: u128,

    // ── Fee breakdown ───────────────────────────────────────────────────────
    /// Portion of the fee claimable by LPs (per-position fees).
    pub claiming_fee: u64,

    /// Portion of the fee collected by the protocol (Meteora treasury).
    pub protocol_fee: u64,

    /// Portion of the fee re-added to pool liquidity (compounding mode only).
    pub compounding_fee: u64,

    /// Portion of the fee paid to a referrer (zero unless the swap had one).
    pub referral_fee: u64,

    /// Whether the fee was charged in token A (`true`) or token B (`false`).
    pub fee_token_is_a: bool,
}

impl SwapEvent {
    /// Convenience: total fee charged on this swap, in the unit of whichever
    /// token bore the fee (see `fee_token_is_a`).
    pub fn fee_total(&self) -> u64 {
        self.claiming_fee
            .saturating_add(self.protocol_fee)
            .saturating_add(self.compounding_fee)
            .saturating_add(self.referral_fee)
    }
}
