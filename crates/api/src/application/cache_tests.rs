use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use yog_core::RepositoryError;

use super::{CachePolicy, SharedCache};

fn policy(ttl: Duration, max_capacity: u64) -> CachePolicy {
    CachePolicy {
        name: "test",
        ttl,
        max_capacity,
    }
}

/// Counts its calls, and takes a moment — long enough for concurrent callers
/// to pile up on the same cold key.
async fn counted(calls: Arc<AtomicUsize>) -> Result<u32, RepositoryError> {
    calls.fetch_add(1, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(50)).await;
    Ok(42)
}

/// Fifty concurrent callers on a cold key: one computation, fifty answers.
/// This is the burst that took the pool down.
///
/// Mutation: compute without the cache, and the count is fifty.
#[tokio::test]
async fn concurrent_callers_of_a_cold_key_share_one_computation() {
    let cache = Arc::new(SharedCache::<(), u32>::new(policy(
        Duration::from_secs(30),
        1,
    )));
    let calls = Arc::new(AtomicUsize::new(0));

    let callers = (0..50).map(|_| {
        let (cache, calls) = (cache.clone(), calls.clone());
        tokio::spawn(async move { cache.get_or_compute((), counted(calls)).await })
    });
    let answers = futures_util::future::join_all(callers).await;

    assert!(answers.into_iter().all(|a| a.unwrap().unwrap() == 42));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// Waiters on a computation that fails get its error — one computation,
/// every caller answered at once — and the next caller computes again:
/// nothing failed is kept.
#[tokio::test]
async fn a_failed_computation_reaches_its_waiters_and_is_not_kept() {
    let cache = Arc::new(SharedCache::<(), u32>::new(policy(
        Duration::from_secs(30),
        1,
    )));
    let calls = Arc::new(AtomicUsize::new(0));

    let callers = (0..20).map(|_| {
        let (cache, calls) = (cache.clone(), calls.clone());
        tokio::spawn(async move {
            cache
                .get_or_compute((), async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Err(RepositoryError::Timeout("no slot".into()))
                })
                .await
        })
    });
    let answers = futures_util::future::join_all(callers).await;

    assert!(
        answers
            .into_iter()
            .all(|a| matches!(a.unwrap(), Err(RepositoryError::Timeout(_))))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let next = cache.get_or_compute((), async { Ok(7) }).await;
    assert_eq!(next.unwrap(), 7, "the failure was not kept");
}

/// Past its time to live, a value is computed again. `moka` keeps time with
/// `std::time::Instant`, which `tokio::time::pause` does not move: a real,
/// short time to live, and a real wait.
#[tokio::test]
async fn an_expired_value_is_computed_again() {
    let cache = SharedCache::<(), u32>::new(policy(Duration::from_millis(100), 1));
    let calls = Arc::new(AtomicUsize::new(0));

    cache
        .get_or_compute((), counted(calls.clone()))
        .await
        .unwrap();
    cache
        .get_or_compute((), counted(calls.clone()))
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1, "still fresh");

    tokio::time::sleep(Duration::from_millis(200)).await;
    cache
        .get_or_compute((), counted(calls.clone()))
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2, "expired");
}

/// Keys are independent: a value cached under one does not answer another.
#[tokio::test]
async fn each_key_has_its_own_value() {
    let cache = SharedCache::<u8, u32>::new(policy(Duration::from_secs(30), 2));

    let one = cache.get_or_compute(1, async { Ok(10) }).await.unwrap();
    let two = cache.get_or_compute(2, async { Ok(20) }).await.unwrap();

    assert_eq!((one, two), (10, 20));
}

/// The capacity holds: ten keys into a cache of three leave three. This is
/// what makes the cache safe on keys that are not a fixed set.
///
/// Mutation: build without `max_capacity`, and ten are kept.
#[tokio::test]
async fn the_capacity_holds() {
    let cache = SharedCache::<u8, u32>::new(policy(Duration::from_secs(30), 3));

    for key in 0..10 {
        cache
            .get_or_compute(key, async move { Ok(u32::from(key)) })
            .await
            .unwrap();
    }
    cache.inner.run_pending_tasks().await;

    assert!(
        cache.inner.entry_count() <= 3,
        "{} entries kept for a capacity of 3",
        cache.inner.entry_count()
    );
}
