//! The shared-result cache: concurrent callers of a cold key share one computation, and its
//! result serves everyone until it expires. A failed computation reaches every waiter and is
//! not kept. ⚠️ A waiter
//! that sees 200 computations dropped in a row panics (`moka`'s retry bound); tokio contains
//! it in that request's task.

use std::hash::Hash;
use std::time::Duration;

use yog_core::RepositoryError;

/// How long a shared result is served: thirty seconds are invisible in hourly aggregates.
pub(crate) const SHARED_RESULT_TTL: Duration = Duration::from_secs(30);

/// How one cached result is kept.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CachePolicy {
    /// Shown in `moka`'s own diagnostics.
    pub(crate) name: &'static str,
    pub(crate) ttl: Duration,
    /// The most entries kept; past it, `moka` evicts.
    pub(crate) max_capacity: u64,
}

/// `/api/protocols`: the catalogue, which counts every pool ever seen.
pub(crate) const PROTOCOLS: CachePolicy = CachePolicy {
    name: "protocols",
    ttl: SHARED_RESULT_TTL,
    max_capacity: 1,
};

/// Results by key, each computed once per [`CachePolicy::ttl`], at most
/// [`CachePolicy::max_capacity`] of them kept.
pub(crate) struct SharedCache<K, V> {
    inner: moka::future::Cache<K, V>,
}

impl<K, V> SharedCache<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub(crate) fn new(policy: CachePolicy) -> Self {
        Self {
            inner: moka::future::Cache::builder()
                .name(policy.name)
                .time_to_live(policy.ttl)
                .max_capacity(policy.max_capacity)
                .build(),
        }
    }

    /// The value for `key`: fresh from the cache, or computed by `compute` —
    /// once, however many callers are waiting for it. A failure reaches every
    /// waiting caller and is not kept.
    pub(crate) async fn get_or_compute<Fut>(
        &self,
        key: K,
        compute: Fut,
    ) -> Result<V, RepositoryError>
    where
        Fut: Future<Output = Result<V, RepositoryError>>,
    {
        self.inner
            .try_get_with(key, compute)
            .await
            .map_err(|shared| RepositoryError::clone(&shared))
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
