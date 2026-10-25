//! The valued liquidity moves over a frozen period: a protocol's totals and series, and a
//! pool's moves. Their coverage is the liquidity's, measured from a later hour than the swaps.

use std::sync::Arc;

use yog_core::{
    Page, RepositoryError, RepositoryResult,
    domain::{
        FrozenPeriod, LiquidityFlows, LiquidityMove, LiquidityMoveCursor, LiquidityMoveLookup,
        LiquidityMoveOrder, Pool, Protocol, ProtocolActivityLookup, ProtocolBucket, Window,
    },
};

use super::period::{BucketPeriod, Measured, build_frozen_period};

/// Why a page of a pool's moves is not read.
#[derive(Debug)]
pub(crate) enum PoolMovesError {
    Repository(RepositoryError),
    /// The cursor was taken from another list, or before the last freeze.
    CursorRefused,
}

impl From<RepositoryError> for PoolMovesError {
    fn from(err: RepositoryError) -> Self {
        Self::Repository(err)
    }
}

pub(crate) struct LiquidityMoveService {
    hours: Arc<dyn ProtocolActivityLookup>,
    liquidity_moves: Arc<dyn LiquidityMoveLookup>,
}

impl LiquidityMoveService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        liquidity_moves: Arc<dyn LiquidityMoveLookup>,
    ) -> Self {
        Self {
            hours,
            liquidity_moves,
        }
    }

    /// The period of `protocol`'s liquidity: its coverage starts at the first hour the liquidity
    /// was measured, not at the facts'.
    async fn build_liquidity_period(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Option<FrozenPeriod>> {
        let Some(period) = build_frozen_period(self.hours.as_ref(), window, None).await? else {
            return Ok(None);
        };
        let since = self.liquidity_moves.find_measured_since(protocol).await?;
        // ⚠️ Not measured yet: covered from `as_of` on, so no period reads as covered.
        let covered_since = since.map_or(period.as_of, |since| since.max(period.covered_since));
        Ok(Some(FrozenPeriod {
            covered_since,
            ..period
        }))
    }

    pub(crate) async fn get_liquidity_totals(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<LiquidityFlows>>> {
        let period = self.build_liquidity_period(window, protocol).await?;
        let items = match &period {
            Some(period) if period.is_covered() => self
                .liquidity_moves
                .find_totals(period, protocol)
                .await?
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn list_liquidity_buckets(
        &self,
        window: Window,
        bucket: BucketPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<LiquidityFlows>>>> {
        let period = self.build_liquidity_period(window, protocol).await?;
        let items = match (&period, bucket) {
            (Some(period), BucketPeriod::Hour) => {
                self.liquidity_moves.list_hourly(period, protocol).await?
            }
            (Some(period), BucketPeriod::Day) => {
                self.liquidity_moves.list_daily(period, protocol).await?
            }
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// A pool's valued moves; an empty page over a period its protocol's liquidity does not
    /// cover whole.
    pub(crate) async fn list_pool_liquidity_moves(
        &self,
        window: Window,
        pool: &Pool,
        order: LiquidityMoveOrder,
        cursor: Option<LiquidityMoveCursor>,
        limit: i64,
    ) -> Result<Measured<Page<LiquidityMove>>, PoolMovesError> {
        let period = self.build_liquidity_period(window, pool.protocol).await?;
        if let (Some(period), Some(cursor)) = (&period, &cursor)
            && (cursor.as_of != period.as_of
                || cursor.window != window
                || cursor.pool != pool.pool_address)
        {
            return Err(PoolMovesError::CursorRefused);
        }
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.liquidity_moves
                    .list_pool_moves(period, window, pool.pool_address, order, cursor, limit)
                    .await?
            }
            _ => Page::empty(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/liquidity_move_service_tests.rs"]
mod tests;
