//! The TVL, a reading at an instant: each protocol's reading of an hour, never a sum.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::domain::swap_activity::sum_known;
use crate::domain::{Protocol, ProtocolSwapActivity};

/// One protocol's TVL as last measured during the hour that closes at `at`: its pools
/// valued by their priced sides; `pools_unvalued` counts those with no side valued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolTvl {
    pub protocol: Protocol,
    pub at: DateTime<Utc>,
    pub tvl_usd: Option<Decimal>,
    pub pools_priced: i64,
    pub pools_unvalued: i64,
}

/// A TVL reading: the protocol's TVL as measured during the hour that closes at `at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TvlReading {
    pub at: DateTime<Utc>,
    pub tvl_usd: Option<Decimal>,
}

/// One protocol's TVL over a window: its last reading, the one a window before it, and the part
/// of the last held by the pools discovered in the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolTvlChange {
    pub protocol: Protocol,
    pub end: TvlReading,
    pub start: Option<TvlReading>,
    pub discovered_tvl_usd: Option<Decimal>,
}

impl ProtocolTvlChange {
    /// A last reading alone: no start, no discovered part, so no change.
    pub fn last_reading(reading: ProtocolTvl) -> Self {
        Self {
            protocol: reading.protocol,
            end: TvlReading {
                at: reading.at,
                tvl_usd: reading.tvl_usd,
            },
            start: None,
            discovered_tvl_usd: None,
        }
    }

    /// The change without the discovered part; `None` as soon as one term is unknown.
    pub fn change_usd(&self) -> Option<Decimal> {
        let start = self.start.as_ref()?.tvl_usd?;
        Some(self.end.tvl_usd? - self.discovered_tvl_usd? - start)
    }

    /// The rows summed: a field unknown in one row is unknown; `None` without rows.
    pub fn total(rows: &[Self]) -> Option<TvlChangeTotal> {
        (!rows.is_empty()).then(|| TvlChangeTotal {
            end_tvl_usd: sum_known(rows.iter().map(|row| row.end.tvl_usd)),
            start_tvl_usd: sum_known(
                rows.iter()
                    .map(|row| row.start.as_ref().and_then(|start| start.tvl_usd)),
            ),
            discovered_tvl_usd: sum_known(rows.iter().map(|row| row.discovered_tvl_usd)),
            change_usd: sum_known(rows.iter().map(Self::change_usd)),
        })
    }
}

/// The protocols' TVL over a window, summed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TvlChangeTotal {
    pub end_tvl_usd: Option<Decimal>,
    pub start_tvl_usd: Option<Decimal>,
    pub discovered_tvl_usd: Option<Decimal>,
    pub change_usd: Option<Decimal>,
}

/// How many times the liquidity deposited changed hands: the volume of a window over the last TVL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiquidityTurnover {
    pub turnover: Option<Decimal>,
    pub pool_hours: i64,
    pub pool_hours_valued: i64,
}

impl LiquidityTurnover {
    /// `None` when a term is unknown, or the TVL is not above 0.
    fn ratio(volume_usd: Option<Decimal>, tvl_usd: Option<Decimal>) -> Option<Decimal> {
        let tvl_usd = tvl_usd.filter(|tvl| tvl.is_sign_positive() && !tvl.is_zero())?;
        volume_usd?.checked_div(tvl_usd)
    }

    /// Every row's volume over every reading: not the sum of the rows' ratios.
    pub fn total(swaps: &[ProtocolSwapActivity], tvl: &[ProtocolTvl]) -> Option<Self> {
        let total = ProtocolSwapActivity::total(swaps)?;
        let tvl_usd = if tvl.is_empty() {
            None
        } else {
            sum_known(tvl.iter().map(|reading| reading.tvl_usd))
        };
        Some(Self {
            turnover: Self::ratio(total.activity.volume_usd, tvl_usd),
            pool_hours: total.activity.pool_hours,
            pool_hours_valued: total.activity.pool_hours_valued,
        })
    }
}

/// One protocol's [`LiquidityTurnover`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolLiquidityTurnover {
    pub protocol: Protocol,
    pub turnover: LiquidityTurnover,
    /// The close of the hour of the reading divided by; `None` without one.
    pub tvl_at: Option<DateTime<Utc>>,
}

impl ProtocolLiquidityTurnover {
    /// One row per row of `swaps`, its TVL `None` without a reading of its protocol.
    pub fn rows(swaps: &[ProtocolSwapActivity], tvl: &[ProtocolTvl]) -> Vec<Self> {
        swaps
            .iter()
            .map(|row| {
                let reading = tvl.iter().find(|reading| reading.protocol == row.protocol);
                Self {
                    protocol: row.protocol,
                    tvl_at: reading.map(|reading| reading.at),
                    turnover: LiquidityTurnover {
                        turnover: LiquidityTurnover::ratio(
                            row.activity.volume_usd,
                            reading.and_then(|reading| reading.tvl_usd),
                        ),
                        pool_hours: row.activity.pool_hours,
                        pool_hours_valued: row.activity.pool_hours_valued,
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
