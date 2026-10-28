//! The life cycle of a protocol's positions over a frozen period: the changes yog-analytic counted.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        PositionChangeCounts, PositionChangeCountsLookup, Protocol, ProtocolActivityLookup, Window,
    },
};

use super::period::{Measured, build_frozen_period};

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
    ) -> RepositoryResult<Measured<Vec<PositionChangeCounts>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match &period {
            Some(period) if period.is_covered() => self
                .counts
                .find_totals(period, protocol)
                .await?
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/position_change_counts_service_tests.rs"]
mod tests;
