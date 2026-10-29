use async_trait::async_trait;

use crate::RepositoryResult;
use crate::domain::{FrozenHours, WrittenTranches};

/// Read lens on yog-analytic's hours, the api's: every measure takes a
/// [`FrozenPeriod`](crate::domain::FrozenPeriod) built from [`Self::frozen_hours`].
#[async_trait]
pub trait ProtocolActivityLookup: Send + Sync {
    /// `None` before the first freeze.
    async fn frozen_hours(&self) -> RepositoryResult<Option<FrozenHours>>;

    /// `None` before the first tranche is written: the period of [`Window::Hour`](crate::domain::Window).
    async fn written_tranches(&self) -> RepositoryResult<Option<WrittenTranches>>;
}
