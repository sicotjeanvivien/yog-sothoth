//! The rotation runs: a protocol's or a pool's list and a protocol's totals through the last
//! tranche written, and its volume in runs by frozen hour.

use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};
use yog_core::{
    Page, RepositoryError, RepositoryResult,
    domain::{
        Protocol, ProtocolActivityLookup, ProtocolBucket, RUN_GAP, RotationRun, RotationRunCursor,
        RotationRunLookup, RotationRunOrder, RotationRunScope, RotationRunTotals, RunSplit,
        TranchePeriod, Window,
    },
};

use super::period::{BucketPeriod, Measured, build_frozen_period, first_whole_hour};

/// How far behind the last tranche written a cursor's end stays accepted, for pages read slowly.
const CURSOR_END_MARGIN: TimeDelta = TimeDelta::hours(1);

/// Why a page of runs is not read.
#[derive(Debug)]
pub(crate) enum RotationRunsError {
    Repository(RepositoryError),
    /// The cursor's end is past the last tranche written, or more than the margin behind it.
    CursorRefused,
}

impl From<RepositoryError> for RotationRunsError {
    fn from(err: RepositoryError) -> Self {
        Self::Repository(err)
    }
}

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

    /// `window` through `as_of`, else through the last tranche written; covered a gap after the
    /// first tranche measured, a run drawn sooner having maybe begun unmeasured.
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
        Ok(Some(TranchePeriod::of(
            as_of,
            window,
            since.map(|since| since + RUN_GAP),
        )))
    }

    /// The first whole hour a gap after the first tranche measured: the bars split from it.
    async fn find_split_since(
        &self,
        protocol: Protocol,
    ) -> RepositoryResult<Option<DateTime<Utc>>> {
        Ok(self
            .runs
            .find_measured_since(protocol)
            .await?
            .map(|since| first_whole_hour(since + RUN_GAP)))
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
    ) -> Result<Measured<Page<RotationRun>, TranchePeriod>, RotationRunsError> {
        let as_of = match &cursor {
            Some(c) => {
                let through = self.runs.find_written_through().await?;
                match through {
                    Some(through)
                        if c.as_of <= through && c.as_of >= through - CURSOR_END_MARGIN =>
                    {
                        Some(c.as_of)
                    }
                    _ => return Err(RotationRunsError::CursorRefused),
                }
            }
            None => None,
        };
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
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<RunSplit>>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match (&period, bucket) {
            (Some(period), BucketPeriod::Hour) => {
                let since = self.find_split_since(protocol).await?;
                self.runs.list_hourly(period, since, protocol).await?
            }
            (Some(period), BucketPeriod::Day) => {
                let since = self.find_split_since(protocol).await?;
                self.runs.list_daily(period, since, protocol).await?
            }
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/rotation_run_service_tests.rs"]
mod tests;
