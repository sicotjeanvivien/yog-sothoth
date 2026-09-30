//! Materialisation backlog repository trait.
//!
//! Reads, for every continuous aggregate, where its materialisation ends and
//! the oldest raw row still waiting past it. It belongs to no CRUD repository —
//! it spans all the aggregates at once — so it stands on its own, like
//! [`EventFreshnessRepository`](crate::domain::EventFreshnessRepository).

use async_trait::async_trait;

use super::MaterializationBacklog;
use crate::RepositoryResult;

/// Reads what each continuous aggregate still has to materialise.
#[async_trait]
pub trait MaterializationBacklogRepository: Send + Sync {
    /// One backlog per continuous aggregate the database holds, the ones added
    /// after this code was written included.
    async fn backlogs(&self) -> RepositoryResult<Vec<MaterializationBacklog>>;
}
