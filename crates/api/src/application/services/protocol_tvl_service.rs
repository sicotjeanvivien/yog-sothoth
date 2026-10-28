//! The TVL, a reading at an instant: the last reading by the last frozen hour, one reading per
//! hour of a window, and its change over a window.

use std::sync::Arc;

use chrono::TimeDelta;
use yog_core::{
    RepositoryResult,
    domain::{
        Protocol, ProtocolActivityLookup, ProtocolTvl, ProtocolTvlChange, ProtocolTvlLookup, Window,
    },
};

use super::period::{Measured, build_frozen_period};

pub(crate) struct ProtocolTvlService {
    hours: Arc<dyn ProtocolActivityLookup>,
    protocol_tvl: Arc<dyn ProtocolTvlLookup>,
}

impl ProtocolTvlService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        protocol_tvl: Arc<dyn ProtocolTvlLookup>,
    ) -> Self {
        Self {
            hours,
            protocol_tvl,
        }
    }

    /// Each protocol's last TVL reading by the last frozen hour.
    pub(crate) async fn get_tvl(
        &self,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolTvl>>> {
        let period = build_frozen_period(self.hours.as_ref(), Window::Day, None).await?;
        let items = match &period {
            Some(period) => {
                self.protocol_tvl
                    .list_latest(period.last_hour(), protocol)
                    .await?
            }
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
                self.protocol_tvl
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

    /// Each protocol's change over `window`, to its last reading; nothing over a window the facts
    /// do not cover whole.
    pub(crate) async fn get_tvl_change(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolTvlChange>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.protocol_tvl
                    .list_changes(period.last_hour(), window, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/protocol_tvl_service_tests.rs"]
mod tests;
