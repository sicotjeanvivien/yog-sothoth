//! The rotation runs: a protocol's or a pool's list and a protocol's totals through the last
//! tranche written, and its volume in runs by frozen hour.

use std::sync::Arc;

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use rust_decimal::Decimal;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        FrozenPeriod, Protocol, ProtocolActivityLookup, ProtocolBucket, RotationRun,
        RotationRunCursor, RotationRunLookup, RotationRunOrder, RotationRunScope,
        RotationRunTotals, TranchePeriod, Window,
    },
};

use super::period::{BucketPeriod, Measured, build_frozen_period};

pub(crate) struct RotationRunService {
    hours: Arc<dyn ProtocolActivityLookup>,
    runs: Arc<dyn RotationRunLookup>,
}

impl RotationRunService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        runs: Arc<dyn RotationRunLookup>,
    ) -> Self {
        Self { hours, runs }
    }

    /// `window` through `as_of`, else through the last tranche written; covered from the first
    /// tranche whose rotation was measured.
    async fn build_tranche_period(
        &self,
        window: Window,
        protocol: Protocol,
        as_of: Option<DateTime<Utc>>,
    ) -> RepositoryResult<Option<TranchePeriod>> {
        let as_of = match as_of {
            Some(as_of) => as_of,
            None => match self.runs.find_written_through().await? {
                Some(through) => through,
                None => return Ok(None),
            },
        };
        let since = self.runs.find_measured_since(protocol).await?;
        Ok(Some(TranchePeriod::of(as_of, window, since)))
    }

    /// The frozen period of the bars: covered from the first whole hour of the rotation.
    async fn build_bar_period(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Option<FrozenPeriod>> {
        let Some(period) = build_frozen_period(self.hours.as_ref(), window, None).await? else {
            return Ok(None);
        };
        let since = self
            .runs
            .find_measured_since(protocol)
            .await?
            .map(first_whole_hour);
        // ⚠️ Not measured yet: covered from `as_of` on, so no period reads as covered.
        let covered_since = since.map_or(period.as_of, |since| since.max(period.covered_since));
        Ok(Some(FrozenPeriod {
            covered_since,
            ..period
        }))
    }

    pub(crate) async fn get_totals(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<RotationRunTotals>, TranchePeriod>> {
        let period = self.build_tranche_period(window, protocol, None).await?;
        let items = match &period {
            Some(period) if period.is_covered() => {
                vec![self.runs.find_totals(period, protocol).await?]
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// `scope`'s runs, `protocol` being its own; a later page keeps the end its cursor carries.
    pub(crate) async fn list_runs(
        &self,
        window: Window,
        scope: RotationRunScope,
        protocol: Protocol,
        order: RotationRunOrder,
        cursor: Option<RotationRunCursor>,
        limit: i64,
    ) -> RepositoryResult<Measured<Page<RotationRun>, TranchePeriod>> {
        let as_of = cursor.as_ref().map(|c| c.as_of);
        let period = self.build_tranche_period(window, protocol, as_of).await?;
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.runs
                    .list_runs(period, window, scope, order, cursor, limit)
                    .await?
            }
            _ => Page::empty(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn list_buckets(
        &self,
        window: Window,
        bucket: BucketPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<Decimal>>>> {
        let period = self.build_bar_period(window, protocol).await?;
        let items = match (&period, bucket) {
            (Some(period), BucketPeriod::Hour) => self.runs.list_hourly(period, protocol).await?,
            (Some(period), BucketPeriod::Day) => self.runs.list_daily(period, protocol).await?,
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

/// The first hour that starts at or after `at`: an hour measured from its start.
fn first_whole_hour(at: DateTime<Utc>) -> DateTime<Utc> {
    let start = at.duration_trunc(TimeDelta::hours(1)).unwrap_or(at);
    if start == at {
        at
    } else {
        start + TimeDelta::hours(1)
    }
}

#[cfg(test)]
#[path = "tests/rotation_run_service_tests.rs"]
mod tests;
