//! The produced resources read by scope — every protocol, one protocol, one pool — each over a
//! span ending at the close of the last frozen hour, as [`ActivityService`] builds it.
//!
//! [`ActivityService`]: super::ActivityService

use std::sync::Arc;

use chrono::TimeDelta;
use solana_pubkey::Pubkey;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        ByProtocol, LiquidityMove, LiquidityMoveCursor, LiquidityMoveLookup, LiquidityMoveOrder,
        LiquidityMoveSums, Pool, PoolCatalog, PoolCreationCount, PoolCreationLookup,
        PoolDiscoveries, PoolDiscoveryLookup, PoolSwapBar, Protocol, ProtocolActivityLookup,
        ProtocolBar, ProtocolSwapTotals, ProtocolTvl, Span, SwapLookup, SwapSums, TvlLookup,
        Window,
    },
};

use super::Measured;

/// The lenses [`MeasureService`] reads.
pub(crate) struct MeasureLenses {
    /// Only for the frozen hours every span is built from.
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

    async fn span(&self, window: Window, offset: Option<Window>) -> RepositoryResult<Option<Span>> {
        Ok(self
            .lenses
            .hours
            .frozen_hours()
            .await?
            .map(|hours| Span::of(hours, window, offset)))
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

    /// Totals: nothing for a span the facts do not cover whole.
    pub(crate) async fn swap_totals(
        &self,
        window: Window,
        offset: Option<Window>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolSwapTotals>>> {
        let span = self.span(window, offset).await?;
        let items = match &span {
            Some(span) if span.is_covered() => {
                self.lenses
                    .swaps
                    .protocol_totals(span, window, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    pub(crate) async fn swap_bars(
        &self,
        window: Window,
        cut: Cut,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolBar<SwapSums>>>> {
        let span = self.span(window, None).await?;
        let items = match (&span, cut) {
            (Some(span), Cut::Hourly) => self.lenses.swaps.protocol_hourly(span, protocol).await?,
            (Some(span), Cut::Daily) => self.lenses.swaps.protocol_daily(span, protocol).await?,
            (None, _) => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    pub(crate) async fn pool_swap_totals(
        &self,
        window: Window,
        pool: Pubkey,
    ) -> RepositoryResult<Measured<Vec<SwapSums>>> {
        let span = self.span(window, None).await?;
        let items = match &span {
            Some(span) if span.is_covered() => self
                .lenses
                .swaps
                .pool_totals(span, pool)
                .await?
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    pub(crate) async fn pool_swap_bars(
        &self,
        window: Window,
        cut: Cut,
        pool: Pubkey,
    ) -> RepositoryResult<Measured<Vec<PoolSwapBar>>> {
        let span = self.span(window, None).await?;
        let items = match (&span, cut) {
            (Some(span), Cut::Hourly) => self.lenses.swaps.pool_hourly(span, pool).await?,
            (Some(span), Cut::Daily) => self.lenses.swaps.pool_daily(span, pool).await?,
            (None, _) => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    pub(crate) async fn pool_creation_totals(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ByProtocol<PoolCreationCount>>>> {
        let span = self.span(window, None).await?;
        let items = match &span {
            Some(span) if span.is_covered() => {
                self.lenses.pool_creations.totals(span, protocol).await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    pub(crate) async fn pool_creation_bars(
        &self,
        window: Window,
        cut: Cut,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolBar<PoolCreationCount>>>> {
        let span = self.span(window, None).await?;
        let items = match (&span, cut) {
            (Some(span), Cut::Hourly) => self.lenses.pool_creations.hourly(span, protocol).await?,
            (Some(span), Cut::Daily) => self.lenses.pool_creations.daily(span, protocol).await?,
            (None, _) => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    /// The span of `protocol`'s liquidity: its coverage starts at the first hour the liquidity
    /// was measured, not at the facts'.
    async fn liquidity_span(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Option<Span>> {
        let Some(span) = self.span(window, None).await? else {
            return Ok(None);
        };
        let since = self.lenses.liquidity_moves.measured_since(protocol).await?;
        // ⚠️ Not measured yet: covered from `as_of` on, so no span reads as covered.
        let covered_since = since.map_or(span.as_of, |since| since.max(span.covered_since));
        Ok(Some(Span {
            covered_since,
            ..span
        }))
    }

    pub(crate) async fn liquidity_totals(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<LiquidityMoveSums>>> {
        let span = self.liquidity_span(window, protocol).await?;
        let items = match &span {
            Some(span) if span.is_covered() => self
                .lenses
                .liquidity_moves
                .totals(span, protocol)
                .await?
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    pub(crate) async fn liquidity_bars(
        &self,
        window: Window,
        cut: Cut,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<ProtocolBar<LiquidityMoveSums>>>> {
        let span = self.liquidity_span(window, protocol).await?;
        let items = match (&span, cut) {
            (Some(span), Cut::Hourly) => self.lenses.liquidity_moves.hourly(span, protocol).await?,
            (Some(span), Cut::Daily) => self.lenses.liquidity_moves.daily(span, protocol).await?,
            (None, _) => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    /// A pool's valued moves; an empty page over a span its protocol's liquidity does not
    /// cover whole.
    pub(crate) async fn pool_liquidity_moves(
        &self,
        window: Window,
        pool: &Pool,
        order: LiquidityMoveOrder,
        cursor: Option<LiquidityMoveCursor>,
        limit: i64,
    ) -> RepositoryResult<PoolMovesRead> {
        let span = self.liquidity_span(window, pool.protocol).await?;
        if let (Some(span), Some(cursor)) = (&span, &cursor)
            && (cursor.as_of != span.as_of
                || cursor.window != window
                || cursor.pool != pool.pool_address)
        {
            return Ok(PoolMovesRead::CursorRefused);
        }
        let items = match &span {
            Some(span) if span.is_covered() => {
                self.lenses
                    .liquidity_moves
                    .pool_moves(span, window, pool.pool_address, order, cursor, limit)
                    .await?
            }
            _ => Page::empty(),
        };
        Ok(PoolMovesRead::Page(Measured { span, items }))
    }

    /// Each protocol's last TVL reading by the last frozen hour.
    pub(crate) async fn tvl(
        &self,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolTvl>>> {
        let span = self.span(Window::Day, None).await?;
        let items = match &span {
            Some(span) => self.lenses.tvl.latest(span.last_hour(), protocol).await?,
            None => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    /// The readings from the window's start through `as_of`: the first is the start's.
    pub(crate) async fn tvl_hourly(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolTvl>>> {
        let span = self.span(window, None).await?;
        let items = match &span {
            Some(span) => {
                self.lenses
                    .tvl
                    .hourly(span.from - TimeDelta::hours(1), span.last_hour(), protocol)
                    .await?
            }
            None => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    pub(crate) async fn pool_discoveries(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<PoolDiscoveries>>> {
        let span = self.span(window, None).await?;
        let items = match &span {
            Some(span) if span.is_covered() => {
                self.lenses
                    .pool_discoveries
                    .totals(span.last_hour(), window, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { span, items })
    }
}

#[cfg(test)]
#[path = "tests/measure_service_tests.rs"]
mod tests;
