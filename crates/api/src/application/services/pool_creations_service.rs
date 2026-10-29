//! The pools created over a frozen period, for every protocol or one protocol.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        PoolCreations, PoolCreationsLookup, Protocol, ProtocolActivityLookup, ProtocolBucket,
        ProtocolPoolCreations, Window,
    },
};

use super::period::{
    BucketPeriod, Measured, ReadPeriod, SeriesInterval, build_frozen_period, build_last_hour,
    build_read_period,
};

pub(crate) struct PoolCreationsService {
    hours: Arc<dyn ProtocolActivityLookup>,
    pool_creations: Arc<dyn PoolCreationsLookup>,
}

impl PoolCreationsService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        pool_creations: Arc<dyn PoolCreationsLookup>,
    ) -> Self {
        Self {
            hours,
            pool_creations,
        }
    }

    /// Totals: nothing for a period the facts do not cover whole.
    pub(crate) async fn get_pool_creation_totals(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolPoolCreations>, ReadPeriod>> {
        let period = build_read_period(self.hours.as_ref(), window, None).await?;
        let items = match &period {
            Some(ReadPeriod::Frozen(p)) if p.is_covered() => {
                self.pool_creations.list_totals(p, protocol).await?
            }
            Some(ReadPeriod::Tranche(p)) if p.is_covered() => {
                self.pool_creations
                    .list_totals_in_tranches(p, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// The series of `window` at `interval`, under the rules of the swaps' series.
    pub(crate) async fn list_pool_creation_series(
        &self,
        window: Window,
        interval: SeriesInterval,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<PoolCreations>>, ReadPeriod>> {
        let Some(bucket) = interval.frozen_bucket() else {
            let period = build_last_hour(self.hours.as_ref()).await?;
            let items = match &period {
                Some(p) => self.pool_creations.list_tranches(p, protocol).await?,
                None => Vec::new(),
            };
            let period = period.map(ReadPeriod::Tranche);
            return Ok(Measured { period, items });
        };
        let measured = self
            .list_pool_creation_buckets(window, bucket, protocol)
            .await?;
        Ok(Measured {
            period: measured.period.map(ReadPeriod::Frozen),
            items: measured.items,
        })
    }

    pub(crate) async fn list_pool_creation_buckets(
        &self,
        window: Window,
        bucket: BucketPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<PoolCreations>>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match (&period, bucket) {
            (Some(period), BucketPeriod::Hour) => {
                self.pool_creations.list_hourly(period, protocol).await?
            }
            (Some(period), BucketPeriod::Day) => {
                self.pool_creations.list_daily(period, protocol).await?
            }
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}
