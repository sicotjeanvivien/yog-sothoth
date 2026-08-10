//! Pool analytics — derived metrics over RPC-sourced data.
//!
//! Sits next to the other domain types but represents a distinct
//! class of data: nothing here comes from the chain directly.
//! [`PoolAnalytics`] is computed on demand by combining RPC-sourced
//! tables (`pool_current_state`, `swap_events`) with context tables
//! (`token_metadata`, `token_prices`).
//!
//! No analytics value is ever persisted into an RPC-sourced table.
//! When materialisation becomes necessary for performance, it will
//! land in a dedicated analytics table written by a separate job —
//! never by the indexer or by yog-context.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// USD-denominated metrics for a single pool.
///
/// The USD fields are `Option` because the inputs may not be fully
/// available:
///
///   - `tvl_usd` is `None` if the pool has no current state yet,
///     or if either token has no known price.
///   - `volume_24h_usd` is `None` only when *no* hour of the window
///     could be valued at all. A partially covered window is returned
///     as `Some(sum_of_valued_hours)` — we surface what we have rather
///     than collapse the value.
///   - `fees_24h_usd` is the realized trading fee over the same 24h
///     window, valued at trade-time prices exactly like volume, and
///     `None` under the same partial-coverage rules. Its three shares
///     — `lp_fees_24h_usd`, `protocol_fees_24h_usd`,
///     `referral_fees_24h_usd` — sum back to it exactly, and are
///     `None` together with it.
///
/// # Why the fee split is read, not derived
///
/// cp-amm takes the referral out of the PROTOCOL share, not the LP one
/// (`cp-amm/src/state/fee.rs::split_fees`):
///
/// ```text
/// protocol_fee_brut = fee_amount × protocol_fee_percent / 100
/// trading_fee       = fee_amount − protocol_fee_brut   → claiming + compounding
/// referral_fee      = protocol_fee_brut × referral_fee_percent / 100
/// protocol_fee      = protocol_fee_brut − referral_fee   ← what is emitted
/// ```
///
/// So the LP share is `claiming + compounding`, NOT
/// `fees - protocol_fees` — that formula credits the referral to the
/// liquidity providers, which is the defect `.project` ticket 05
/// records. The split is therefore computed once, in
/// `meteora_damm_v2_pool_hourly_activity`, and carried here as three
/// read values. Do not re-derive one of them from the others.
///
/// The effective fee rate is still left to the presentation layer:
/// `fees_24h_usd / volume_24h_usd` is a ratio, not a share.
///
/// # Why the two bucket counters exist
///
/// Surfacing a partial sum is defensible; surfacing it *silently* is
/// not. `SUM` skips the hours it cannot value, so a window covered at
/// 58 % returned a number indistinguishable from a complete one — a
/// sub-total presented as a total. `swap_buckets_24h` and
/// `swap_buckets_priced_24h` are that missing signal, the same
/// numerator/denominator pattern as `GlobalAnalytics::pools_priced`:
/// hours with at least one swap, and how many of them the USD
/// valuation actually covered. They apply to the **five swap-derived**
/// fields above at once — `volume_24h_usd` and the four fee figures —
/// because those share one valuation, so an hour lost to one is lost
/// to all. ⚠️ Not `tvl_usd`: it is a stock, valued at the latest price
/// rather than per bucket, and these counters say nothing about it.
///
/// The denominator counts hours *with swaps*, not hours with any
/// activity: an hour holding only a liquidity event is not a volume
/// coverage failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolAnalytics {
    pub tvl_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub fees_24h_usd: Option<Decimal>,
    /// The three shares of `fees_24h_usd`, summing back to it exactly.
    /// `protocol` is what cp-amm emits (already net of the referral),
    /// `referral` the referrer's cut of the protocol share, and `lp`
    /// what is left for the liquidity providers (`claiming +
    /// compounding`) — see the type doc for why `lp` is not
    /// `fees - protocol`.
    pub protocol_fees_24h_usd: Option<Decimal>,
    pub referral_fees_24h_usd: Option<Decimal>,
    pub lp_fees_24h_usd: Option<Decimal>,
    /// Hours of the 24h window that had at least one swap.
    pub swap_buckets_24h: i64,
    /// …of which the USD valuation covered. Never greater than
    /// `swap_buckets_24h`; equal to it means full coverage.
    pub swap_buckets_priced_24h: i64,
}

impl PoolAnalytics {
    /// Sentinel for "no analytics computable for this pool", used
    /// to fill the gaps when a requested pool address is missing
    /// from the repository batch result.
    pub fn empty() -> Self {
        Self {
            tvl_usd: None,
            volume_24h_usd: None,
            fees_24h_usd: None,
            protocol_fees_24h_usd: None,
            referral_fees_24h_usd: None,
            lp_fees_24h_usd: None,
            swap_buckets_24h: 0,
            swap_buckets_priced_24h: 0,
        }
    }
}

/// One hourly bucket of a pool's activity history, USD-denominated.
///
/// Built from the four hourly continuous aggregates (swap, liquidity,
/// claim_position_fee, claim_reward) joined on the bucket, each valued at the
/// token price as-of that bucket (trade-time valuation, like [`PoolAnalytics`]).
///
/// Every metric is `Option` because a bucket may have activity from one source
/// but not another, and because USD valuation needs a known price for the
/// tokens involved at that time — `None` means "no priced activity of this kind
/// in this bucket", surfaced rather than coerced to zero.
///
/// A bucket with a non-null `swap_count` and a null `volume_usd` is therefore
/// meaningful and expected: the hour traded and could not be valued. It is the
/// per-bucket form of the coverage counters on [`PoolAnalytics`], which is why
/// this type needs none of its own — a caller derives coverage by counting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolHistoryBucket {
    /// Start of the hourly bucket (UTC).
    pub bucket: DateTime<Utc>,
    pub volume_usd: Option<Decimal>,
    /// Realized trading fee from swaps, and its three shares — same
    /// definition and same caveat as on [`PoolAnalytics`]: they sum
    /// back to `fees_usd`, and `lp_fees_usd` is NOT
    /// `fees_usd - protocol_fees_usd`.
    pub fees_usd: Option<Decimal>,
    pub protocol_fees_usd: Option<Decimal>,
    pub referral_fees_usd: Option<Decimal>,
    pub lp_fees_usd: Option<Decimal>,
    pub liquidity_added_usd: Option<Decimal>,
    pub liquidity_removed_usd: Option<Decimal>,
    /// LP position fees actually claimed in this bucket.
    pub fees_claimed_usd: Option<Decimal>,
    /// Farming rewards actually claimed in this bucket (summed across mints).
    pub rewards_claimed_usd: Option<Decimal>,
    /// Number of swaps in the bucket.
    pub swap_count: Option<i64>,
}
