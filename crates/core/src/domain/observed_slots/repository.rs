//! Where the slots each ingestion session watched are written.

use async_trait::async_trait;

use crate::{RepositoryResult, domain::ObservedSlots};

/// Write side, the indexer's lens.
#[async_trait]
pub trait ObservedSlotsRepository: Send + Sync {
    /// Record a session's slots: a row only ever widens, a narrower write leaves it as it is.
    async fn record(&self, observed: &ObservedSlots) -> RepositoryResult<()>;
}
