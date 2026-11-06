//! The keys the API accepts; there is no write side here, keys are an operator's (`yog-migrate`).

use async_trait::async_trait;

use crate::{RepositoryResult, domain::ActiveKey};

/// The keys, the api's lens.
#[async_trait]
pub trait ApiKeyLookup: Send + Sync {
    /// Every key not revoked, of a client not revoked.
    async fn active_keys(&self) -> RepositoryResult<Vec<ActiveKey>>;
}
