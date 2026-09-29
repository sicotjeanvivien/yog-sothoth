//! Materialisation progress repository trait.
//!
//! Reads, for every continuous aggregate, where its materialisation ends and
//! where its raw rows are. It belongs to no CRUD repository — it spans all
//! the aggregates at once — so it stands on its own, like
//! [`EventFreshnessRepository`](crate::domain::EventFreshnessRepository).

use async_trait::async_trait;

use super::AggregateMaterialization;
use crate::RepositoryResult;

/// Reads how far each continuous aggregate trails its raw rows.
#[async_trait]
pub trait MaterializationRepository: Send + Sync {
    /// One entry per continuous aggregate the database holds, the ones added
    /// after this code was written included.
    async fn progress(&self) -> RepositoryResult<Vec<AggregateMaterialization>>;
}
