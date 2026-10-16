use async_trait::async_trait;

use crate::{RepositoryResult, domain::GlobalAnalytics};

/// Read-only access to protocol-wide analytics.
#[async_trait]
pub trait GlobalAnalyticsRepository: Send + Sync {
    /// See [`GlobalAnalytics`].
    async fn global_analytics(&self) -> RepositoryResult<GlobalAnalytics>;
}
