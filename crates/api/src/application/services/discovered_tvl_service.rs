//! The part of each protocol's TVL held by the pools discovered in a window.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        DiscoveredTvlLookup, Protocol, ProtocolActivityLookup, ProtocolDiscoveredTvl, Window,
    },
};

use super::period::{Measured, build_frozen_period};

pub(crate) struct DiscoveredTvlService {
    hours: Arc<dyn ProtocolActivityLookup>,
    discovered_tvl: Arc<dyn DiscoveredTvlLookup>,
}

impl DiscoveredTvlService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        discovered_tvl: Arc<dyn DiscoveredTvlLookup>,
    ) -> Self {
        Self {
            hours,
            discovered_tvl,
        }
    }

    /// Nothing over a window the facts do not cover whole.
    pub(crate) async fn get_discovered_tvl(
        &self,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolDiscoveredTvl>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.discovered_tvl
                    .list_totals(period.last_hour(), window, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/discovered_tvl_service_tests.rs"]
mod tests;
