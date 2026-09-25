use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::TtlCache;

/// Counts its calls, and takes a moment — long enough for concurrent callers
/// to pile up on the same cold key.
async fn counted(calls: &AtomicUsize) -> Result<u32, String> {
    calls.fetch_add(1, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(50)).await;
    Ok(42)
}

/// Fifty concurrent callers on a cold key: one computation, fifty answers.
/// This is the burst that took the pool down.
///
/// Mutation: give each caller its own cell, and the count is fifty.
#[tokio::test]
async fn concurrent_callers_of_a_cold_key_share_one_computation() {
    let cache = Arc::new(TtlCache::<(), u32>::new(Duration::from_secs(30)));
    let calls = Arc::new(AtomicUsize::new(0));

    let callers = (0..50).map(|_| {
        let (cache, calls) = (cache.clone(), calls.clone());
        tokio::spawn(async move { cache.get_or_try_compute((), || counted(&calls)).await })
    });
    let answers = futures_util::future::join_all(callers).await;

    assert!(answers.into_iter().all(|a| a.unwrap() == Ok(42)));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// Past its time to live, a value is computed again.
#[tokio::test(start_paused = true)]
async fn an_expired_value_is_computed_again() {
    let cache = TtlCache::<(), u32>::new(Duration::from_secs(30));
    let calls = AtomicUsize::new(0);

    cache
        .get_or_try_compute((), || counted(&calls))
        .await
        .unwrap();
    tokio::time::advance(Duration::from_secs(29)).await;
    cache
        .get_or_try_compute((), || counted(&calls))
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1, "still fresh at 29 s");

    tokio::time::advance(Duration::from_secs(2)).await;
    cache
        .get_or_try_compute((), || counted(&calls))
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2, "expired at 31 s");
}

/// A failure is not served to the next caller: it computes again.
#[tokio::test]
async fn an_error_is_not_cached() {
    let cache = TtlCache::<(), u32>::new(Duration::from_secs(30));

    let first: Result<u32, String> = cache
        .get_or_try_compute((), || async { Err("db down".to_string()) })
        .await;
    let second: Result<u32, String> = cache.get_or_try_compute((), || async { Ok(7) }).await;

    assert_eq!(first, Err("db down".to_string()));
    assert_eq!(second, Ok(7));
}

/// Keys are independent: a value cached under one does not answer another.
#[tokio::test]
async fn each_key_has_its_own_value() {
    let cache = TtlCache::<u8, u32>::new(Duration::from_secs(30));

    let one = cache
        .get_or_try_compute(1, || async { Ok::<_, String>(10) })
        .await;
    let two = cache
        .get_or_try_compute(2, || async { Ok::<_, String>(20) })
        .await;

    assert_eq!((one, two), (Ok(10), Ok(20)));
}
