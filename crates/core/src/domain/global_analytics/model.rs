//! Protocol-wide analytics over every observed pool, with no per-pool identity.

use rust_decimal::Decimal;

/// Protocol-wide USD sums of what is priceable, with their coverage: `pools_priced`
/// for the TVL, the swap buckets for volume and fees.
/// ⚠️ Without the coverage a partial sum reads exactly like a complete one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalAnalytics {
    pub total_tvl_usd: Option<Decimal>,
    pub pools_priced: i64,
    pub volume_24h_usd: Option<Decimal>,
    pub fees_24h_usd: Option<Decimal>,
    pub swap_buckets_24h: i64,
    pub swap_buckets_priced_24h: i64,
}
