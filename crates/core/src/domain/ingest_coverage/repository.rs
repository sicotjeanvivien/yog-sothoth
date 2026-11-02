//! Contract of the ingestion coverage journal.

use async_trait::async_trait;

use crate::{RepositoryResult, domain::ObservedSlots};

/// Write side of the journal, the indexer's lens.
#[async_trait]
pub trait IngestCoverageRepository: Send + Sync {
    /// Record a session's range: a row only ever widens, a narrower write leaves it as it is.
    async fn record(&self, observed: &ObservedSlots) -> RepositoryResult<()>;
}
