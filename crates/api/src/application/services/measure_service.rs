//! The produced resources read by scope — every protocol, one protocol, one pool — each over a
//! period ending at the close of the last frozen hour, as [`ActivityService`] builds it.
//!
//! [`ActivityService`]: super::ActivityService

use std::sync::Arc;

use chrono::TimeDelta;
use solana_pubkey::Pubkey;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        ByProtocol, FrozenPeriod, LiquidityMove, LiquidityMoveCursor, LiquidityMoveLookup,
        LiquidityMoveOrder, LiquidityMoveSums, Pool, PoolCatalog, PoolCreationCount,
        PoolCreationLookup, PoolDiscoveries, PoolDiscoveryLookup, PoolSwapBar, Protocol,
        ProtocolActivityLookup, ProtocolBar, ProtocolSwapTotals, ProtocolTvl, SwapLookup, SwapSums,
        TvlLookup, Window,
    },
};

use super::Measured;

/// The lenses [`MeasureService`] reads.
pub(crate) struct MeasureLenses {
    /// Only for the frozen hours every period is built from.
    pub(crate) hours: Arc<dyn ProtocolActivityLookup>,
    pub(crate) swaps: Arc<dyn SwapLookup>,
    pub(crate) pool_creations: Arc<dyn PoolCreationLookup>,
    pub(crate) liquidity_moves: Arc<dyn LiquidityMoveLookup>,
    pub(crate) tvl: Arc<dyn TvlLookup>,
    pub(crate) pool_discoveries: Arc<dyn PoolDiscoveryLookup>,
    pub(crate) pools: Arc<dyn PoolCatalog>,
}

/// A page of a pool's moves, or the refusal of a cursor taken from another list or before
/// the last freeze.
#[derive(Debug)]
pub(crate) enum PoolMovesRead {
    Page(Measured<Page<LiquidityMove>>),
    CursorRefused,
}

/// The bars of a series: one per hour, or one per 24 hours ending at `as_of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cut {
    Hourly,
    Daily,
}

pub(crate) struct MeasureService {
    lenses: MeasureLenses,
}

impl MeasureService {
    pub(crate) fn new(lenses: MeasureLenses) -> Self {
        Self { lenses }
    }

    async fn period(
        &self,
        window: Window,
        offset: Option<Window>,
    ) -> RepositoryResult<Option<FrozenPeriod>> {
        Ok(self
            .lenses
            .hours
            .frozen_hours()
            .await?
            .map(|hours| FrozenPeriod::of(hours, window, offset)))
    }

    /// The pool at `address`, `None` when the catalog does not know it.
    pub(crate) async fn pool(&self, address: Pubkey) -> RepositoryResult<Option<Pool>> {
        Ok(self
            .lenses
            .pools
            .find_by_addresses(&[address])
            .await?
            .into_iter()
            .find(|pool| pool.pool_address == address))
    }

    /// Totals: nothing for a period the facts do not cover whole.
    pub(crate) async fn swap_totals(
        &self,
        window: Window,
        offset: Option<Window>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolSwapTotals>>> {
        let period = self.period(window, offset).await?;
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.lenses
                    .swaps
                    .protocol_totals(period, window, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn swap_bars(
        &self,
        window: Window,
        cut: Cut,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolBar<SwapSums>>>> {
        let period = self.period(window, None).await?;
        let items = match (&period, cut) {
            (Some(period), Cut::Hourly) => {
                self.lenses.swaps.protocol_hourly(period, protocol).await?
            }
            (Some(period), Cut::Daily) => {
                self.lenses.swaps.protocol_daily(period, protocol).await?
            }
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn pool_swap_totals(
        &self,
        window: Window,
        pool: Pubkey,
    ) -> RepositoryResult<Measured<Vec<SwapSums>>> {
        let period = self.period(window, None).await?;
        let items = match &period {
            Some(period) if period.is_covered() => self
                .lenses
                .swaps
                .pool_totals(period, pool)
                .await?
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn pool_swap_bars(
        &self,
        window: Window,
        cut: Cut,
        pool: Pubkey,
    ) -> RepositoryResult<Measured<Vec<PoolSwapBar>>> {
        let period = self.period(window, None).await?;
        let items = match (&period, cut) {
            (Some(period), Cut::Hourly) => self.lenses.swaps.pool_hourly(period, pool).await?,
            (Some(period), Cut::Daily) => self.lenses.swaps.pool_daily(period, pool).await?,
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn pool_creation_totals(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ByProtocol<PoolCreationCount>>>> {
        let period = self.period(window, None).await?;
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.lenses.pool_creations.totals(period, protocol).await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn pool_creation_bars(
        &self,
        window: Window,
        cut: Cut,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolBar<PoolCreationCount>>>> {
        let period = self.period(window, None).await?;
        let items = match (&period, cut) {
            (Some(period), Cut::Hourly) => {
                self.lenses.pool_creations.hourly(period, protocol).await?
            }
            (Some(period), Cut::Daily) => {
                self.lenses.pool_creations.daily(period, protocol).await?
            }
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// The period of `protocol`'s liquidity: its coverage starts at the first hour the liquidity
    /// was measured, not at the facts'.
    async fn liquidity_span(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Option<FrozenPeriod>> {
        let Some(period) = self.period(window, None).await? else {
            return Ok(None);
        };
        let since = self.lenses.liquidity_moves.measured_since(protocol).await?;
        // ⚠️ Not measured yet: covered from `as_of` on, so no period reads as covered.
        let covered_since = since.map_or(period.as_of, |since| since.max(period.covered_since));
        Ok(Some(FrozenPeriod {
            covered_since,
            ..period
        }))
    }

    pub(crate) async fn liquidity_totals(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<LiquidityMoveSums>>> {
        let period = self.liquidity_span(window, protocol).await?;
        let items = match &period {
            Some(period) if period.is_covered() => self
                .lenses
                .liquidity_moves
                .totals(period, protocol)
                .await?
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn liquidity_bars(
        &self,
        window: Window,
        cut: Cut,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<ProtocolBar<LiquidityMoveSums>>>> {
        let period = self.liquidity_span(window, protocol).await?;
        let items = match (&period, cut) {
            (Some(period), Cut::Hourly) => {
                self.lenses.liquidity_moves.hourly(period, protocol).await?
            }
            (Some(period), Cut::Daily) => {
                self.lenses.liquidity_moves.daily(period, protocol).await?
            }
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// A pool's valued moves; an empty page over a period its protocol's liquidity does not
    /// cover whole.
    pub(crate) async fn pool_liquidity_moves(
        &self,
        window: Window,
        pool: &Pool,
        order: LiquidityMoveOrder,
        cursor: Option<LiquidityMoveCursor>,
        limit: i64,
    ) -> RepositoryResult<PoolMovesRead> {
        let period = self.liquidity_span(window, pool.protocol).await?;
        if let (Some(period), Some(cursor)) = (&period, &cursor)
            && (cursor.as_of != period.as_of
                || cursor.window != window
                || cursor.pool != pool.pool_address)
        {
            return Ok(PoolMovesRead::CursorRefused);
        }
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.lenses
                    .liquidity_moves
                    .pool_moves(period, window, pool.pool_address, order, cursor, limit)
                    .await?
            }
            _ => Page::empty(),
        };
        Ok(PoolMovesRead::Page(Measured { period, items }))
    }

    /// Each protocol's last TVL reading by the last frozen hour.
    pub(crate) async fn tvl(
        &self,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolTvl>>> {
        let period = self.period(Window::Day, None).await?;
        let items = match &period {
            Some(period) => self.lenses.tvl.latest(period.last_hour(), protocol).await?,
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// The readings from the window's start through `as_of`: the first is the start's.
    pub(crate) async fn tvl_hourly(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolTvl>>> {
        let period = self.period(window, None).await?;
        let items = match &period {
            Some(period) => {
                self.lenses
                    .tvl
                    .hourly(
                        period.from - TimeDelta::hours(1),
                        period.last_hour(),
                        protocol,
                    )
                    .await?
            }
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn pool_discoveries(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<PoolDiscoveries>>> {
        let period = self.period(window, None).await?;
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.lenses
                    .pool_discoveries
                    .totals(period.last_hour(), window, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/measure_service_tests.rs"]
mod tests;
