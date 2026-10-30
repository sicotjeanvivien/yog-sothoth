//! The life cycle of a protocol's positions over a period: the changes yog-analytic counted, the
//! last hour counted on the events themselves.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        PositionChangeCounts, PositionChangeCountsLookup, Protocol, ProtocolActivityLookup,
        ProtocolBucket, Window,
    },
};

use super::period::{BucketPeriod, Measured, ReadPeriod, SeriesInterval, build_read_period};

pub(crate) struct PositionChangeCountsService {
    hours: Arc<dyn ProtocolActivityLookup>,
    counts: Arc<dyn PositionChangeCountsLookup>,
}

impl PositionChangeCountsService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        counts: Arc<dyn PositionChangeCountsLookup>,
    ) -> Self {
        Self { hours, counts }
    }

    /// `protocol`'s position changes over `window`; nothing over a window the facts do not cover.
    pub(crate) async fn get_position_change_totals(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<PositionChangeCounts>, ReadPeriod>> {
        let period = build_read_period(self.hours.as_ref(), window, None).await?;
        let items = match &period {
            Some(ReadPeriod::Frozen(p)) if p.is_covered() => {
                self.counts.find_totals(p, protocol).await?
            }
            Some(ReadPeriod::Tranche(p)) if p.is_covered() => {
                self.counts.find_totals_in_tranches(p, protocol).await?
            }
            _ => None,
        };
        Ok(Measured {
            period,
            items: items.into_iter().collect(),
        })
    }

    /// `protocol`'s position changes of `window` by `interval`, from the facts' coverage.
    pub(crate) async fn list_position_change_series(
        &self,
        window: Window,
        interval: SeriesInterval,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<PositionChangeCounts>>, ReadPeriod>> {
        let period = build_read_period(self.hours.as_ref(), window, None).await?;
        let items = match (&period, interval.frozen_bucket()) {
            (Some(ReadPeriod::Tranche(p)), None) => self.counts.list_tranches(p, protocol).await?,
            (Some(ReadPeriod::Frozen(p)), Some(BucketPeriod::Hour)) => {
                self.counts.list_hourly(p, protocol).await?
            }
            (Some(ReadPeriod::Frozen(p)), Some(bucket)) => {
                self.counts
                    .list_buckets(p, bucket.width(), protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/position_change_counts_service_tests.rs"]
mod tests;
