//! How many times the liquidity deposited changed hands: the volume of a window over the last TVL.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        LiquidityTurnover, Protocol, ProtocolActivityLookup, ProtocolLiquidityTurnover,
        ProtocolTvlLookup, SwapActivityLookup, Window,
    },
};

use super::period::{Measured, ReadPeriod, build_frozen_period, build_read_period};

pub(crate) struct LiquidityTurnoverService {
    hours: Arc<dyn ProtocolActivityLookup>,
    swap_activity: Arc<dyn SwapActivityLookup>,
    protocol_tvl: Arc<dyn ProtocolTvlLookup>,
}

/// The turnovers of a window; `total` is `None` without a row.
#[derive(Debug)]
pub(crate) struct TurnoverRead {
    pub(crate) measured: Measured<Vec<ProtocolLiquidityTurnover>, ReadPeriod>,
    pub(crate) total: Option<LiquidityTurnover>,
}

impl LiquidityTurnoverService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        swap_activity: Arc<dyn SwapActivityLookup>,
        protocol_tvl: Arc<dyn ProtocolTvlLookup>,
    ) -> Self {
        Self {
            hours,
            swap_activity,
            protocol_tvl,
        }
    }

    /// Nothing for a period the facts do not cover whole. The TVL is the last frozen hour's,
    /// whatever the window: it is a stock, not a sum over it.
    pub(crate) async fn get_totals(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<TurnoverRead> {
        let period = build_read_period(self.hours.as_ref(), window, None).await?;
        let swaps = match &period {
            Some(ReadPeriod::Frozen(p)) if p.is_covered() => {
                self.swap_activity
                    .find_protocol_totals(p, window, protocol)
                    .await?
            }
            Some(ReadPeriod::Tranche(p)) if p.is_covered() => {
                self.swap_activity
                    .find_protocol_totals_in_tranches(p, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        let frozen = build_frozen_period(self.hours.as_ref(), Window::Day, None).await?;
        let tvl = match (&frozen, swaps.is_empty()) {
            (Some(frozen), false) => {
                self.protocol_tvl
                    .list_latest(frozen.last_hour(), protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(TurnoverRead {
            total: LiquidityTurnover::total(&swaps, &tvl),
            measured: Measured {
                period,
                items: ProtocolLiquidityTurnover::rows(&swaps, &tvl),
            },
        })
    }
}

#[cfg(test)]
#[path = "tests/liquidity_turnover_service_tests.rs"]
mod tests;
