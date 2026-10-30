//! The rotation runs: a protocol's or a pool's list and a protocol's totals through the last
//! tranche written, and its volume in runs by frozen hour.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use yog_core::{
    Page, RepositoryError, RepositoryResult,
    domain::{
        FrozenPeriod, Protocol, ProtocolActivityLookup, ProtocolBucket, RUN_GAP, RotationRun,
        RotationRunCursor, RotationRunLookup, RotationRunOrder, RotationRunScope,
        RotationRunTotals, RunSplit, TranchePeriod, Window,
    },
};

use super::period::{
    BucketPeriod, CURSOR_END_MARGIN, Measured, ReadPeriod, SeriesInterval, build_frozen_period,
    build_last_hour, first_whole_hour,
};

/// The volume the runs split, by bar, and the first instant they split a bar from.
#[derive(Debug)]
pub(crate) struct RunSplitSeries<P> {
    pub(crate) measured: Measured<Vec<ProtocolBucket<RunSplit>>, P>,
    pub(crate) split_since: Option<DateTime<Utc>>,
}

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
    ) -> RepositoryResult<RunSplitSeries<FrozenPeriod>> {
        let Some(period) = build_frozen_period(self.hours.as_ref(), window, None).await? else {
            return Ok(RunSplitSeries {
                measured: Measured {
                    period: None,
                    items: Vec::new(),
                },
                split_since: None,
            });
        };
        let since = self.find_split_since(protocol).await?;
        let items = match bucket {
            BucketPeriod::Hour => self.runs.list_hourly(&period, since, protocol).await?,
            bucket => {
                self.runs
                    .list_buckets(&period, bucket.width(), since, protocol)
                    .await?
            }
        };
        Ok(RunSplitSeries {
            measured: Measured {
                period: Some(period),
                items,
            },
            split_since: since,
        })
    }

    /// The volume of `window` by `interval`, split by the runs; the last hour by tranche, split
    /// from a gap after the first tranche measured.
    pub(crate) async fn list_series(
        &self,
        window: Window,
        interval: SeriesInterval,
        protocol: Protocol,
    ) -> RepositoryResult<RunSplitSeries<ReadPeriod>> {
        let Some(bucket) = interval.frozen_bucket() else {
            let period = build_last_hour(self.hours.as_ref()).await?;
            let since = self.runs.find_measured_since(protocol).await?;
            let split_since = since.map(|since| since + RUN_GAP);
            let items = match &period {
                Some(p) => self.runs.list_tranches(p, split_since, protocol).await?,
                None => Vec::new(),
            };
            let period = period.map(ReadPeriod::Tranche);
            return Ok(RunSplitSeries {
                measured: Measured { period, items },
                split_since,
            });
        };
        let series = self.list_buckets(window, bucket, protocol).await?;
        Ok(RunSplitSeries {
            measured: Measured {
                period: series.measured.period.map(ReadPeriod::Frozen),
                items: series.measured.items,
            },
            split_since: series.split_since,
        })
    }
}

#[cfg(test)]
#[path = "tests/rotation_run_service_tests.rs"]
mod tests;
