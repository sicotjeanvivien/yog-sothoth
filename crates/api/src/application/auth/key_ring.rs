//! The keys the API accepts, held in memory: a key the ring does not hold never reaches the
//! database, whatever the number presented.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use yog_core::{
    RepositoryResult,
    domain::{ActiveKey, ApiClient, ApiKey, ApiKeyLookup},
};

use super::log;
use crate::application::{ReloadOutcome, metrics::ApiMetrics};

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
        let keys = lookup.active_keys().await?;
        Ok(Self::with_keys(lookup, keys))
    }

    /// A ring holding `keys`, which it rereads from `lookup`.
    pub(crate) fn with_keys(lookup: Arc<dyn ApiKeyLookup>, keys: Vec<ActiveKey>) -> Self {
        Self {
            keys: Arc::new(RwLock::new(Arc::new(by_hash(keys)))),
            lookup,
        }
    }

    /// The client `key` opens the API to, if any.
    pub(crate) fn client_of(&self, key: &ApiKey) -> Option<ApiClient> {
        self.current().get(&key.hash()).cloned()
    }

    /// Reread the keys; ⚠️ a failed read keeps the ring as it was, rather than refuse every key.
    pub(crate) async fn reload(&self) -> ReloadOutcome {
        let outcome = match self.lookup.active_keys().await {
            Ok(keys) => {
                *self
                    .keys
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::new(by_hash(keys));
                ReloadOutcome::Reloaded
            }
            Err(error) => {
                log::not_reloaded(&error);
                ReloadOutcome::Failed
            }
        };
        ApiMetrics::record_key_ring_reload(outcome);
        outcome
    }

    fn current(&self) -> Arc<Keys> {
        self.keys
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

fn by_hash(keys: Vec<ActiveKey>) -> Keys {
    keys.into_iter().map(|key| (key.hash, key.client)).collect()
}

#[cfg(test)]
#[path = "key_ring_tests.rs"]
mod tests;
