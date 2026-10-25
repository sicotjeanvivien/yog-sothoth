use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::ProtocolDiscoveredTvl;
use crate::RepositoryResult;
use crate::domain::{Protocol, Window};

/// The pools discovered in a window, read with the TVL reading they were measured at.
#[async_trait]
pub trait DiscoveredTvlLookup: Send + Sync {
    /// Each protocol's discoveries for `window` at its last reading at or before the hour `last`.
    async fn list_totals(
        &self,
        last: DateTime<Utc>,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolDiscoveredTvl>>;
}
