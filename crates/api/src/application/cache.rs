//! The shared-result cache, for results that are the same for every caller.
//!
//! `/api/pools/top` and `/api/stats` answer every visitor with the same body,
//! and each took 2–4 s to compute (measured 25 September 2026). Computing it
//! once per request let 30 parallel requests from one client take every
//! database connection. Here, concurrent callers of a cold key share **one**
//! computation, and its result serves everyone until it expires.
//!
//! # Adding a cached result
//!
//! Declare its [`CachePolicy`] below — a name, a time to live, a capacity, and
//! the reason for each — then hold a [`SharedCache`] built from it in the
//! service, and take a work slot *inside* the computation (see
//! `PoolService::top_pools`), never around the call: a caller waiting for a
//! cached value must not hold a slot.
//!
//! # What the cache does, measured
//!
//! Built on `moka`, checked in a throwaway project before adopting it
//! (September 2026):
//! - concurrent callers of a cold key share one computation;
//! - a failed computation's error reaches every caller that was waiting for
//!   it, and nothing is kept — the next caller computes again;
//! - if the caller running the computation is dropped (its client gone, or
//!   past `REQUEST_TIMEOUT`), the computation goes with it and a waiting
//!   caller starts it again. At 3.7–4.4 s per computation that costs one
//!   redone computation; on a database slow enough to push one past the 10 s
//!   deadline, the cached routes answer `503` until it recovers, like the
//!   other slow routes;
//! - ⚠️ a caller that sees **200** computations in a row dropped while it
//!   waits panics (`moka`'s retry bound). It takes 200 clients opening a cold
//!   key and leaving, one after the other, while one request waits; tokio
//!   contains the panic in that request's connection task and the process
//!   carries on.

use std::hash::Hash;
use std::time::Duration;

use yog_core::RepositoryError;

/// How long a shared result is served before it is computed again.
///
/// The value the deployment already gives `Cache-Control` on `/api/pools`.
/// What is cached reads hourly aggregates and a 24 h window: thirty seconds
/// of staleness is invisible in it.
pub(crate) const SHARED_RESULT_TTL: Duration = Duration::from_secs(30);

/// How one cached result is kept: named once, here, with its reasons.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CachePolicy {
    /// Shown in `moka`'s own diagnostics.
    pub(crate) name: &'static str,
    pub(crate) ttl: Duration,
    /// The most entries kept; past it, `moka` evicts. What makes a cache safe
    /// on keys that are not a fixed set.
    pub(crate) max_capacity: u64,
}

/// `/api/pools/top`: one ranking per metric, computed at the largest `limit`
/// and cut to each request's.
pub(crate) const TOP_POOLS: CachePolicy = CachePolicy {
    name: "top_pools",
    ttl: SHARED_RESULT_TTL,
    // Three metrics: volume, TVL, fees.
    max_capacity: 3,
};

/// `/api/stats`: one aggregate for the whole protocol.
pub(crate) const STATS: CachePolicy = CachePolicy {
    name: "stats",
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
        // `moka` shares the error behind an `Arc`; `RepositoryError` is `Clone`
        // so every caller gets it back as its own.
        self.inner
            .try_get_with(key, compute)
            .await
            .map_err(|shared| RepositoryError::clone(&shared))
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
