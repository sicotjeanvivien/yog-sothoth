//! The swaps' measures over a frozen period: for every protocol, one protocol, or one pool.

use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    RepositoryResult,
    domain::{
        PoolSwapBucket, Protocol, ProtocolActivityLookup, ProtocolBucket, ProtocolSwapActivity,
        SwapActivity, SwapActivityLookup, Window,
    },
};

use super::period::{BucketPeriod, Measured, build_frozen_period};

pub(crate) struct SwapActivityService {
    hours: Arc<dyn ProtocolActivityLookup>,
    swap_activity: Arc<dyn SwapActivityLookup>,
}

impl SwapActivityService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        swap_activity: Arc<dyn SwapActivityLookup>,
    ) -> Self {
        Self {
            hours,
            swap_activity,
        }
    }

    /// Totals: nothing for a period the facts do not cover whole.
    pub(crate) async fn get_swap_totals(
        &self,
        window: Window,
        offset: Option<Window>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolSwapActivity>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, offset).await?;
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.swap_activity
                    .find_protocol_totals(period, window, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn list_swap_buckets(
        &self,
        window: Window,
        bucket: BucketPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<SwapActivity>>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match (&period, bucket) {
            (Some(period), BucketPeriod::Hour) => {
                self.swap_activity
                    .list_protocol_hourly(period, protocol)
                    .await?
            }
            (Some(period), BucketPeriod::Day) => {
                self.swap_activity
                    .list_protocol_daily(period, protocol)
                    .await?
            }
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn get_pool_swap_totals(
        &self,
        window: Window,
        pool: Pubkey,
    ) -> RepositoryResult<Measured<Vec<SwapActivity>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match &period {
            Some(period) if period.is_covered() => self
                .swap_activity
                .find_pool_totals(period, pool)
                .await?
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn list_pool_swap_buckets(
        &self,
        window: Window,
        bucket: BucketPeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Measured<Vec<PoolSwapBucket>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match (&period, bucket) {
            (Some(period), BucketPeriod::Hour) => {
                self.swap_activity.list_pool_hourly(period, pool).await?
            }
            (Some(period), BucketPeriod::Day) => {
                self.swap_activity.list_pool_daily(period, pool).await?
            }
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/swap_activity_service_tests.rs"]
mod tests;
