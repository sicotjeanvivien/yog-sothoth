//! The keys the API accepts, held in memory and reloaded on a period: a key the ring does not
//! hold never reaches the database, whatever the number presented.

use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, RwLock},
    time::Duration,
};

use tokio_util::sync::CancellationToken;
use tracing::warn;
use yog_core::{
    RepositoryResult,
    domain::{ApiClient, ApiKey, ApiKeyLookup},
};

use crate::application::metrics::{ApiMetrics, KeyRingReload};

/// How often the ring rereads the keys: a revocation takes effect within it.
pub(crate) const KEY_RING_RELOAD: Duration = Duration::from_secs(10);

type Keys = HashMap<[u8; 32], ApiClient>;

/// Every active key, by hash, with its client.
#[derive(Clone)]
pub(crate) struct KeyRing {
    keys: Arc<RwLock<Arc<Keys>>>,
    lookup: Arc<dyn ApiKeyLookup>,
}

impl KeyRing {
    /// The ring as the database holds it now; a failure here stops the start.
    pub(crate) async fn load(lookup: Arc<dyn ApiKeyLookup>) -> RepositoryResult<Self> {
        let keys = read(lookup.as_ref()).await?;
        Ok(Self {
            keys: Arc::new(RwLock::new(Arc::new(keys))),
            lookup,
        })
    }

    /// The client `key` opens the API to, if any.
    pub(crate) fn client_of(&self, key: &ApiKey) -> Option<ApiClient> {
        self.current().get(&key.hash()).cloned()
    }

    /// Reread the keys; ⚠️ a failed read keeps the ring as it was, rather than refuse every key.
    pub(crate) async fn reload(&self) -> RepositoryResult<()> {
        let keys = read(self.lookup.as_ref()).await?;
        *self
            .keys
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::new(keys);
        Ok(())
    }

    /// Reload every `period` until `shutdown`.
    pub(crate) async fn run(
        self,
        period: Duration,
        shutdown: CancellationToken,
    ) -> Result<(), Infallible> {
        loop {
            tokio::select! {
                () = shutdown.cancelled() => return Ok(()),
                () = tokio::time::sleep(period) => {}
            }
            // ⚠️ Raced too: a read waiting on a saturated pool would otherwise hold the stop.
            let reloaded = tokio::select! {
                () = shutdown.cancelled() => return Ok(()),
                reloaded = self.reload() => reloaded,
            };
            match reloaded {
                Ok(()) => ApiMetrics::record_key_ring_reload(KeyRingReload::Reloaded),
                Err(error) => {
                    warn!(%error, "api keys not reloaded — the ring keeps the keys it held");
                    ApiMetrics::record_key_ring_reload(KeyRingReload::Failed);
                }
            }
        }
    }

    /// A ring holding `keys` and rereading them, for a test with no database.
    #[cfg(test)]
    pub(crate) fn holding(keys: Vec<yog_core::domain::ActiveKey>) -> Self {
        let lookup = Arc::new(FixedKeys(keys.clone()));
        Self {
            keys: Arc::new(RwLock::new(Arc::new(
                keys.into_iter().map(|k| (k.hash, k.client)).collect(),
            ))),
            lookup,
        }
    }

    fn current(&self) -> Arc<Keys> {
        self.keys
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

/// The same keys on every read.
#[cfg(test)]
struct FixedKeys(Vec<yog_core::domain::ActiveKey>);

#[cfg(test)]
#[async_trait::async_trait]
impl ApiKeyLookup for FixedKeys {
    async fn active_keys(&self) -> RepositoryResult<Vec<yog_core::domain::ActiveKey>> {
        Ok(self.0.clone())
    }
}

async fn read(lookup: &dyn ApiKeyLookup) -> RepositoryResult<Keys> {
    Ok(lookup
        .active_keys()
        .await?
        .into_iter()
        .map(|key| (key.hash, key.client))
        .collect())
}

#[cfg(test)]
#[path = "api_keys_tests.rs"]
mod tests;
