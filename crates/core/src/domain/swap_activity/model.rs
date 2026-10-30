//! The swaps yog-analytic measured, read by scope: every protocol, one protocol, or one pool.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::domain::{Protocol, ProtocolBucket};

/// The terms added, `None` as soon as one is unknown or their sum overflows.
pub(crate) fn sum_known(terms: impl IntoIterator<Item = Option<Decimal>>) -> Option<Decimal> {
    terms
        .into_iter()
        .try_fold(Decimal::ZERO, |sum, term| sum.checked_add(term?))
}

/// The swaps of a scope over a period or a bucket. The USD sums skip the pool-hours not
/// valued, which `pool_hours - pool_hours_valued` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapActivity {
    pub swaps: i64,
    pub volume_usd: Option<Decimal>,
    pub fees_usd: Option<Decimal>,
    pub lp_fees_usd: Option<Decimal>,
    pub protocol_fees_usd: Option<Decimal>,
    pub referral_fees_usd: Option<Decimal>,
    pub pool_hours: i64,
    pub pool_hours_valued: i64,
}

/// One protocol's swaps over a period, with its pools that swapped in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSwapActivity {
    pub protocol: Protocol,
    pub activity: SwapActivity,
    /// Distinct over the window; `None` when no cycle wrote it at the period's last hour.
    pub pools: Option<i64>,
}

impl SwapActivity {
    /// The protocols of each bucket summed, the earliest first: each USD field over the rows that
    /// know it; `None` if none does, or on overflow.
    pub fn bucket_totals(rows: &[ProtocolBucket<Self>]) -> Vec<SwapBucket> {
        let mut buckets: BTreeMap<DateTime<Utc>, Vec<&Self>> = BTreeMap::new();
        for row in rows {
            buckets.entry(row.from).or_default().push(&row.value);
        }
        buckets
            .into_iter()
            .map(|(from, values)| {
                let usd = |field: fn(&Self) -> Option<Decimal>| {
                    let mut known = values.iter().filter_map(|value| field(value));
                    let first = known.next()?;
                    known.try_fold(first, Decimal::checked_add)
                };
                SwapBucket {
                    from,
                    activity: Self {
                        swaps: values.iter().map(|value| value.swaps).sum(),
                        volume_usd: usd(|a| a.volume_usd),
                        fees_usd: usd(|a| a.fees_usd),
                        lp_fees_usd: usd(|a| a.lp_fees_usd),
                        protocol_fees_usd: usd(|a| a.protocol_fees_usd),
                        referral_fees_usd: usd(|a| a.referral_fees_usd),
                        pool_hours: values.iter().map(|value| value.pool_hours).sum(),
                        pool_hours_valued: values.iter().map(|value| value.pool_hours_valued).sum(),
                    },
                }
            })
            .collect()
    }
}

/// Every protocol's swaps over one bucket of a series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapBucket {
    pub from: DateTime<Utc>,
    pub activity: SwapActivity,
}

/// The protocols' swaps over a period, summed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapActivityTotal {
    pub activity: SwapActivity,
    pub pools: Option<i64>,
}

impl ProtocolSwapActivity {
    /// The rows summed: a USD field or `pools` unknown in one row is unknown, and a USD field that
    /// overflows; `None` without rows.
    pub fn total(rows: &[Self]) -> Option<SwapActivityTotal> {
        if rows.is_empty() {
            return None;
        }
        let usd = |field: fn(&SwapActivity) -> Option<Decimal>| {
            sum_known(rows.iter().map(|row| field(&row.activity)))
        };
        Some(SwapActivityTotal {
            activity: SwapActivity {
                swaps: rows.iter().map(|row| row.activity.swaps).sum(),
                volume_usd: usd(|a| a.volume_usd),
                fees_usd: usd(|a| a.fees_usd),
                lp_fees_usd: usd(|a| a.lp_fees_usd),
                protocol_fees_usd: usd(|a| a.protocol_fees_usd),
                referral_fees_usd: usd(|a| a.referral_fees_usd),
                pool_hours: rows.iter().map(|row| row.activity.pool_hours).sum(),
                pool_hours_valued: rows.iter().map(|row| row.activity.pool_hours_valued).sum(),
            },
            pools: rows.iter().map(|row| row.pools).sum(),
        })
    }
}

/// One pool's swaps over one bucket of a series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolSwapBucket {
    pub from: DateTime<Utc>,
    pub activity: SwapActivity,
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
