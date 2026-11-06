//! The contracts of the API keys: an operator writes them (`yog-migrate`), the api reads them.

use async_trait::async_trait;

use crate::{
    RepositoryResult,
    domain::{ActiveKey, ApiKey, ApiKeyListing},
};

/// The operator's side: issue, revoke and list the keys.
#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    /// Store `key` for `client`, creating the client on its first key.
    /// ⚠️ A revoked client is refused: its key would be stored and never accepted.
    async fn create(&self, client: &str, key: &ApiKey) -> RepositoryResult<()>;

    /// Revoke the key named by `prefix`; `false` when no key under it was still active.
    async fn revoke(&self, prefix: &str) -> RepositoryResult<bool>;

    /// Every key, revoked or not, the newest first.
    async fn list(&self) -> RepositoryResult<Vec<ApiKeyListing>>;
}

/// The keys, the api's lens.
#[async_trait]
pub trait ApiKeyLookup: Send + Sync {
    /// Every key not revoked, of a client not revoked.
    async fn active_keys(&self) -> RepositoryResult<Vec<ActiveKey>>;
}
