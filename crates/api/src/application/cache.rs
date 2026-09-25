//! A small shared cache for results that are the same for every caller.
//!
//! `/api/pools/top` and `/api/stats` answer every visitor with the same body,
//! and each took 2–4 s to compute (measured 25 September 2026). Computing it
//! once per request let 30 parallel requests from one client take every
//! database connection. Here, concurrent callers of a cold key share **one**
//! computation, and its result serves everyone for [`SHARED_RESULT_TTL`].
//!
//! Written here rather than taken from a crate (`moka` does this): what is
//! needed is one map, one lock held without an `.await`, and tokio's
//! `OnceCell`, which already does the single flight.

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::OnceCell;
use tokio::time::Instant;

/// How long a shared result is served before it is computed again.
///
/// The value the deployment already gives `Cache-Control` on `/api/pools`.
/// What is cached reads hourly aggregates and a 24 h window: thirty seconds
/// of staleness is invisible in it.
pub(crate) const SHARED_RESULT_TTL: Duration = Duration::from_secs(30);

/// Results by key, each computed once per `ttl`.
///
/// ⚠️ **The key space must be bounded.** An expired entry is replaced when its
/// key is asked for again, never swept: a key taken from free user input
/// would grow the map without limit. The callers' keys are an enum, or `()`.
pub(crate) struct TtlCache<K, V, E> {
    ttl: Duration,
    entries: Mutex<HashMap<K, Entry<V, E>>>,
}

struct Entry<V, E> {
    created: Instant,
    /// Shared by every caller that arrives while it is fresh: the first to
    /// reach it computes, the others wait for the same **outcome** — an
    /// error included, as a single flight does.
    cell: Arc<OnceCell<Result<V, E>>>,
}

impl<K: Eq + Hash + Clone, V: Clone, E: Clone> TtlCache<K, V, E> {
    pub(crate) fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// The value for `key`: fresh from the cache, or computed by `compute` —
    /// once, however many callers are waiting for it.
    ///
    /// An error reaches every caller that was waiting for that computation,
    /// then the entry is dropped: the next caller computes again. Retrying
    /// inside the flight instead — what `OnceCell::get_or_try_init` does —
    /// had each waiter redo the computation after the previous one failed,
    /// its own slot wait included: under load, 2, 4, 6… seconds to refuse.
    pub(crate) async fn get_or_try_compute<F, Fut>(&self, key: K, compute: F) -> Result<V, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>>,
    {
        let cell = {
            // A poisoned lock only means a panic elsewhere while it was held;
            // the map itself is always consistent (no await under the lock).
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            let now = Instant::now();
            match entries.get(&key) {
                Some(entry) if now.duration_since(entry.created) < self.ttl => entry.cell.clone(),
                _ => {
                    let cell = Arc::new(OnceCell::new());
                    entries.insert(
                        key.clone(),
                        Entry {
                            created: now,
                            cell: cell.clone(),
                        },
                    );
                    cell
                }
            }
        };

        let outcome = cell.get_or_init(compute).await.clone();
        if outcome.is_err() {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            // Only this flight's entry: a newer one may have replaced it.
            if entries
                .get(&key)
                .is_some_and(|entry| Arc::ptr_eq(&entry.cell, &cell))
            {
                entries.remove(&key);
            }
        }
        outcome
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
