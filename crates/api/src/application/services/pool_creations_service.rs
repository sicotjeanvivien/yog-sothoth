//! The pools created over a frozen period, for every protocol or one protocol.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        PoolCreations, PoolCreationsLookup, Protocol, ProtocolActivityLookup, ProtocolBucket,
        ProtocolPoolCreations, Window,
    },
};

use super::period::{BucketPeriod, Measured, build_frozen_period};

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
    ) -> RepositoryResult<Measured<Vec<ProtocolPoolCreations>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.pool_creations.list_totals(period, protocol).await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
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
