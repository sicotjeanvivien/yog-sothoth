//! The TVL, a reading at an instant: the last reading by the last frozen hour, and one
//! reading per hour of a window.

use std::sync::Arc;

use chrono::TimeDelta;
use yog_core::{
    RepositoryResult,
    domain::{Protocol, ProtocolActivityLookup, ProtocolTvl, TvlLookup, Window},
};

use super::period::{Measured, build_frozen_period};

pub(crate) struct TvlService {
    hours: Arc<dyn ProtocolActivityLookup>,
    tvl: Arc<dyn TvlLookup>,
}

impl TvlService {
    pub(crate) fn new(hours: Arc<dyn ProtocolActivityLookup>, tvl: Arc<dyn TvlLookup>) -> Self {
        Self { hours, tvl }
    }

    /// Each protocol's last TVL reading by the last frozen hour.
    pub(crate) async fn get_tvl(
        &self,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolTvl>>> {
        let period = build_frozen_period(self.hours.as_ref(), Window::Day, None).await?;
        let items = match &period {
            Some(period) => self.tvl.list_latest(period.last_hour(), protocol).await?,
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// The readings from the window's start through `as_of`: the first is the start's.
    pub(crate) async fn list_tvl_hourly(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolTvl>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match &period {
            Some(period) => {
                self.tvl
                    .list_hourly(
                        period.from - TimeDelta::hours(1),
                        period.last_hour(),
                        protocol,
                    )
                    .await?
            }
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/tvl_service_tests.rs"]
mod tests;
