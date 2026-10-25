use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::ProtocolTvl;
use crate::RepositoryResult;
use crate::domain::Protocol;

#[async_trait]
pub trait ProtocolTvlLookup: Send + Sync {
    /// Each protocol's last reading at or before the hour `last`.
    async fn list_latest(
        &self,
        last: DateTime<Utc>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvl>>;

    /// The readings of the hours `first` through `last` that were measured, oldest first.
    async fn list_hourly(
        &self,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvl>>;
}
