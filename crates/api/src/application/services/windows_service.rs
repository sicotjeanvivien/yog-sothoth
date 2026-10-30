//! The windows the service reads, judged without reading a measure over them.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{FrozenPeriod, ProtocolActivityLookup, TranchePeriod, Window},
};

use super::period::ReadPeriod;

pub(crate) struct WindowsService {
    hours: Arc<dyn ProtocolActivityLookup>,
}

impl WindowsService {
    pub(crate) fn new(hours: Arc<dyn ProtocolActivityLookup>) -> Self {
        Self { hours }
    }

    /// Every window, the shortest first: `1h` through the last tranche, the others frozen.
    pub(crate) async fn list(&self) -> RepositoryResult<Vec<(Window, Option<ReadPeriod>)>> {
        let (tranches, hours) =
            tokio::try_join!(self.hours.written_tranches(), self.hours.frozen_hours())?;
        let hour = tranches
            .map(|tranches| ReadPeriod::Tranche(TranchePeriod::hour_through(tranches, None)));
        let frozen = Window::ALL.map(|window| {
            let period =
                hours.map(|hours| ReadPeriod::Frozen(FrozenPeriod::of(hours, window, None)));
            (window, period)
        });
        Ok(std::iter::once((Window::Hour, hour))
            .chain(frozen)
            .collect())
    }
}

#[cfg(test)]
#[path = "tests/windows_service_tests.rs"]
mod tests;
