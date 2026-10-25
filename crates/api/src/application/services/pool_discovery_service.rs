//! The part of each protocol's TVL held by the pools discovered in a window.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        PoolDiscoveryLookup, Protocol, ProtocolActivityLookup, ProtocolDiscoveredTvl, Window,
    },
};

use super::period::{Measured, build_frozen_period};

pub(crate) struct PoolDiscoveryService {
    hours: Arc<dyn ProtocolActivityLookup>,
    pool_discoveries: Arc<dyn PoolDiscoveryLookup>,
}

impl PoolDiscoveryService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        pool_discoveries: Arc<dyn PoolDiscoveryLookup>,
    ) -> Self {
        Self {
            hours,
            pool_discoveries,
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
                self.pool_discoveries
                    .list_totals(period.last_hour(), window, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/pool_discovery_service_tests.rs"]
mod tests;
