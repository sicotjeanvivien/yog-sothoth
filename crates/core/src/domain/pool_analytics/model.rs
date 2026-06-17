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
/// All fields are `Option` because the inputs may not be fully
/// available:
///
///   - `tvl_usd` is `None` if the pool has no current state yet,
///     or if either token has no known price.
///   - `volume_24h_usd` is `None` if the pool has no swaps in the
///     last 24h whose tokens had a known price at the time of the
///     swap. A partial volume (some swaps priced, some not) is
///     returned as `Some(sum_of_priced_swaps)` — we surface what
///     we have rather than collapse the value because of partial
///     coverage.
///   - `fees_24h_usd` / `protocol_fees_24h_usd` are the realized
///     trading fee and Meteora's share of it over the same 24h
///     window, valued at trade-time prices exactly like volume.
///     `None` under the same partial-coverage rules. The LP share
///     is `fees_24h_usd - protocol_fees_24h_usd`; the effective fee
///     rate is `fees_24h_usd / volume_24h_usd` — both left to the
///     presentation layer to derive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolAnalytics {
    pub tvl_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub fees_24h_usd: Option<Decimal>,
    pub protocol_fees_24h_usd: Option<Decimal>,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolHistoryBucket {
    /// Start of the hourly bucket (UTC).
    pub bucket: DateTime<Utc>,
    pub volume_usd: Option<Decimal>,
    /// Realized trading fee and Meteora's cut of it (from swaps).
    pub fees_usd: Option<Decimal>,
    pub protocol_fees_usd: Option<Decimal>,
    pub liquidity_added_usd: Option<Decimal>,
    pub liquidity_removed_usd: Option<Decimal>,
    /// LP position fees actually claimed in this bucket.
    pub fees_claimed_usd: Option<Decimal>,
    /// Farming rewards actually claimed in this bucket (summed across mints).
    pub rewards_claimed_usd: Option<Decimal>,
    /// Number of swaps in the bucket.
    pub swap_count: Option<i64>,
}
