//! Pool analytics: USD metrics derived on demand, never read from the chain.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// USD metrics of one pool.
/// ⚠️ The fee split is read, never derived: cp-amm takes the referral from the
/// protocol share, so `lp` is not `fees - protocol` (`split_fees`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolAnalytics {
    pub tvl_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub fees_24h_usd: Option<Decimal>,
    /// The three shares of `fees_24h_usd`, summing back to it exactly.
    pub protocol_fees_24h_usd: Option<Decimal>,
    pub referral_fees_24h_usd: Option<Decimal>,
    pub lp_fees_24h_usd: Option<Decimal>,
    /// Hours of the 24h window that had at least one swap.
    /// ⚠️ With the next field: coverage of volume and fees only, not of `tvl_usd`.
    pub swap_buckets_24h: i64,
    /// …of which the USD valuation covered.
    pub swap_buckets_priced_24h: i64,
}

impl PoolAnalytics {
    /// "No analytics computable", for a pool missing from a batch result.
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

/// One hourly bucket of a pool's activity, valued at trade-time prices.
/// A `swap_count` with a `None` volume is expected: the hour traded, unvalued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolHistoryBucket {
    /// Start of the hourly bucket (UTC).
    pub bucket: DateTime<Utc>,
    pub volume_usd: Option<Decimal>,
    /// Realized trading fee and its three shares, as on [`PoolAnalytics`].
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
