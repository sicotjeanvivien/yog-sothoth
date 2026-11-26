use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::domain::{Protocol, ProtocolSwapActivity, ProtocolTvl};
use crate::tools::sum_known;

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
