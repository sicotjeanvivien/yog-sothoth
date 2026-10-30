//! The valued liquidity moves: a protocol's totals and series over a period, and the moves of a
//! protocol or a pool over one. Their coverage is the liquidity's, measured from a later hour than
//! the swaps.

use std::sync::Arc;

use chrono::TimeDelta;

use yog_core::{
    Page, RepositoryError, RepositoryResult,
    domain::{
        LiquidityFlows, LiquidityMove, LiquidityMoveCursor, LiquidityMoveList, LiquidityMoveLookup,
        LiquidityMoveOrder, LiquidityTotals, Protocol, ProtocolActivityLookup, ProtocolBucket,
        Window,
    },
};

use super::period::{
    BucketPeriod, CURSOR_END_MARGIN, Measured, PeriodBounds, ReadPeriod, SeriesInterval,
    build_read_period,
};

/// Why a page of moves is not read.
#[derive(Debug)]
pub(crate) enum MovesError {
    Repository(RepositoryError),
    /// The cursor was taken over another window, or its end is past the period's, more than the
    /// margin behind it, or off the period's step.
    CursorRefused,
}

impl From<RepositoryError> for MovesError {
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

    /// The period of `protocol`'s liquidity, the last hour included: its coverage starts at the
    /// first hour the liquidity was measured, not at the facts'.
    async fn build_liquidity_read_period(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Option<ReadPeriod>> {
        let Some(period) = build_read_period(self.hours.as_ref(), window, None).await? else {
            return Ok(None);
        };
        let since = self.liquidity_moves.find_measured_since(protocol).await?;
        Ok(Some(period.measured_from(since)))
    }

    pub(crate) async fn get_liquidity_totals(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<LiquidityTotals>, ReadPeriod>> {
        let period = self.build_liquidity_read_period(window, protocol).await?;
        let items = match &period {
            Some(ReadPeriod::Frozen(p)) if p.is_covered() => self
                .liquidity_moves
                .find_totals(p, protocol)
                .await?
                .into_iter()
                .collect(),
            Some(ReadPeriod::Tranche(p)) if p.is_covered() => vec![
                self.liquidity_moves
                    .find_totals_in_tranches(p, protocol)
                    .await?,
            ],
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn list_liquidity_series(
        &self,
        window: Window,
        interval: SeriesInterval,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<LiquidityFlows>>, ReadPeriod>> {
        let period = self.build_liquidity_read_period(window, protocol).await?;
        let items = match (&period, interval.frozen_bucket()) {
            (Some(ReadPeriod::Tranche(p)), None) => {
                self.liquidity_moves.list_tranches(p, protocol).await?
            }
            (Some(ReadPeriod::Frozen(p)), Some(BucketPeriod::Hour)) => {
                self.liquidity_moves.list_hourly(p, protocol).await?
            }
            (Some(ReadPeriod::Frozen(p)), Some(bucket)) => {
                self.liquidity_moves
                    .list_buckets(p, bucket.width(), protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// `list`'s valued moves; an empty page over a period `protocol`'s liquidity does not cover
    /// whole.
    pub(crate) async fn list_liquidity_moves(
        &self,
        window: Window,
        list: LiquidityMoveList,
        protocol: Protocol,
        order: LiquidityMoveOrder,
        cursor: Option<LiquidityMoveCursor>,
        limit: i64,
    ) -> Result<Measured<Page<LiquidityMove>, ReadPeriod>, MovesError> {
        let period = match (
            self.build_liquidity_read_period(window, protocol).await?,
            &cursor,
        ) {
            (Some(period), Some(c)) => {
                let end = period.as_of();
                let behind = end - c.as_of;
                if c.window != window
                    || behind < TimeDelta::zero()
                    || behind > CURSOR_END_MARGIN
                    || behind.num_seconds() % period.step().num_seconds() != 0
                {
                    return Err(MovesError::CursorRefused);
                }
                Some(period.ending_at(c.as_of))
            }
            (period, _) => period,
        };
        let items = match &period {
            Some(ReadPeriod::Frozen(p)) if p.is_covered() => {
                self.liquidity_moves
                    .list_moves(p, window, list, order, cursor, limit)
                    .await?
            }
            Some(ReadPeriod::Tranche(p)) if p.is_covered() => {
                self.liquidity_moves
                    .list_moves_in_tranches(p, window, list, order, cursor, limit)
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
