//! What every continuous aggregate still has to materialise.

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
