use super::*;

// ── index_concurrency ──────────────────────────────────────────────────
//
// ⚠️ No configuration reaches the guard (`Database::connect`'s pool is fixed),
// which is why it is tested: a refusal nothing reaches is one nobody notices broken.

#[test]
fn a_pool_leaves_one_connection_for_the_reporter() {
    assert_eq!(index_concurrency(10).unwrap(), 9);
    assert_eq!(index_concurrency(2).unwrap(), 1);
}

#[test]
fn a_pool_with_nothing_left_to_reserve_is_refused() {
    // One connection: clamping to 1 would hand the reporter's only connection to an index task.
    let refusal = index_concurrency(1).unwrap_err().to_string();
    assert!(
        refusal.contains("1 connection(s)") && refusal.contains("more connections"),
        "the refusal must name the pool it saw and what to do about it, got: {refusal}"
    );

    assert!(index_concurrency(0).is_err());
}

#[test]
fn the_bound_never_reaches_zero() {
    // `Semaphore::new(0)` would deadlock rather than fail: no accepted pool size may produce it.
    for max_connections in 2..64 {
        assert!(
            index_concurrency(max_connections).unwrap() >= 1,
            "pool of {max_connections} produced a zero bound"
        );
    }
}
