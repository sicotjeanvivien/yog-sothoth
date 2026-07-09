//! Integration test for the paginated signal feed read
//! (`PgSignalRepository::list`).
//!
//! Gated behind `integration-tests`. Validates the vertical slice on the
//! real `signals` hypertable: display order (`triggered_at DESC, id DESC`),
//! forward/backward keyset navigation, boundary flags, the id tie-break on
//! equal timestamps, and the optional severity and pool filters.

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::{
    Cursor, PageDirection, PagePosition,
    domain::{Protocol, Severity, Signal, SignalCursor, SignalFeed, SignalRepository},
};
use yog_persistence::PgSignalRepository;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn signal(pool: u8, severity: Severity, triggered_at: DateTime<Utc>) -> Signal {
    Signal {
        detector: "flow_imbalance".to_string(),
        protocol: Protocol::MeteoraDammV2,
        pool_address: pk(pool),
        severity,
        value: Decimal::new(75, 2),
        threshold: Some(Decimal::new(6, 1)),
        message: None,
        triggered_at,
    }
}

fn ids(page: &yog_core::Page<yog_core::domain::SignalRecord>) -> Vec<i64> {
    page.items.iter().map(|r| r.id).collect()
}

fn signal_cursor(cursor: &Cursor) -> SignalCursor {
    match cursor {
        Cursor::Signal(c) => c.clone(),
        other => panic!("expected a signal cursor, got {other:?}"),
    }
}

/// Seed five signals at strictly increasing timestamps and return the
/// base instant. In a fresh test database the BIGSERIAL assigns ids
/// 1..=5 in insert order, so id N was triggered at `base + N seconds` —
/// newest first is 5,4,3,2,1.
async fn seed_five(repo: &PgSignalRepository) -> DateTime<Utc> {
    let base = Utc::now() - Duration::hours(1);
    let severities = [
        Severity::Warning,  // id 1
        Severity::Critical, // id 2
        Severity::Warning,  // id 3
        Severity::Critical, // id 4
        Severity::Warning,  // id 5
    ];
    let batch: Vec<Signal> = severities
        .iter()
        .enumerate()
        .map(|(i, &sev)| signal(1, sev, base + Duration::seconds(i as i64 + 1)))
        .collect();
    repo.insert_batch(&batch).await.unwrap();
    base
}

#[sqlx::test]
async fn feed_paginates_newest_first(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    seed_five(&repo).await;

    // Page 1: newest two.
    let page1 = repo
        .list(None, None, None, PageDirection::Next, None, 2)
        .await
        .unwrap();
    assert_eq!(ids(&page1), vec![5, 4]);
    assert!(page1.is_first && !page1.is_last);
    assert!(page1.prev_cursor.is_none());

    // Page 2 via the forward cursor.
    let cursor = signal_cursor(page1.next_cursor.as_ref().unwrap());
    let page2 = repo
        .list(None, None, Some(cursor), PageDirection::Next, None, 2)
        .await
        .unwrap();
    assert_eq!(ids(&page2), vec![3, 2]);
    assert!(!page2.is_first && !page2.is_last);

    // Page 3: the tail.
    let cursor = signal_cursor(page2.next_cursor.as_ref().unwrap());
    let page3 = repo
        .list(None, None, Some(cursor), PageDirection::Next, None, 2)
        .await
        .unwrap();
    assert_eq!(ids(&page3), vec![1]);
    assert!(!page3.is_first && page3.is_last);
    assert!(page3.next_cursor.is_none());

    // Backward from page 2 returns page 1, in display order.
    let cursor = signal_cursor(page2.prev_cursor.as_ref().unwrap());
    let back = repo
        .list(None, None, Some(cursor), PageDirection::Prev, None, 2)
        .await
        .unwrap();
    assert_eq!(ids(&back), vec![5, 4]);
    assert!(back.is_first);

    // Position jumps ignore cursors: Last = the oldest page.
    let last = repo
        .list(
            None,
            None,
            None,
            PageDirection::Next,
            Some(PagePosition::Last),
            2,
        )
        .await
        .unwrap();
    assert_eq!(ids(&last), vec![2, 1]);
    assert!(last.is_last);
}

#[sqlx::test]
async fn equal_timestamps_tie_break_on_id_desc(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    let at = Utc::now() - Duration::minutes(5);
    // Two signals in the same tick share triggered_at (ids 1 and 2).
    repo.insert_batch(&[
        signal(1, Severity::Warning, at),
        signal(2, Severity::Warning, at),
    ])
    .await
    .unwrap();

    let page1 = repo
        .list(None, None, None, PageDirection::Next, None, 1)
        .await
        .unwrap();
    assert_eq!(ids(&page1), vec![2]);

    // The cursor must step past the tie deterministically, not skip or
    // repeat the sibling row.
    let cursor = signal_cursor(page1.next_cursor.as_ref().unwrap());
    let page2 = repo
        .list(None, None, Some(cursor), PageDirection::Next, None, 1)
        .await
        .unwrap();
    assert_eq!(ids(&page2), vec![1]);
    assert!(page2.is_last);
}

#[sqlx::test]
async fn latest_cursor_and_newer_than_track_the_feed_tip(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);

    // Empty feed: no tip yet.
    assert!(repo.latest_cursor().await.unwrap().is_none());

    let base = seed_five(&repo).await;

    // The tip is the newest row.
    let tip = repo.latest_cursor().await.unwrap().unwrap();
    assert_eq!(tip.id, 5);

    // Strictly after id 2: chronological (ASC) delivery of 3, 4, 5.
    let after_two = yog_core::domain::SignalCursor {
        triggered_at: base + Duration::seconds(2),
        id: 2,
    };
    let delta = repo.newer_than(&after_two, 10).await.unwrap();
    assert_eq!(
        delta.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![3, 4, 5]
    );

    // The limit caps the batch — the rest stays for the next poll.
    let capped = repo.newer_than(&after_two, 2).await.unwrap();
    assert_eq!(capped.iter().map(|r| r.id).collect::<Vec<_>>(), vec![3, 4]);

    // Nothing is strictly after the tip.
    assert!(repo.newer_than(&tip, 10).await.unwrap().is_empty());
}

#[sqlx::test]
async fn pool_filter_restricts_the_feed(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    let base = Utc::now() - Duration::hours(1);
    // Interleaved pools: ids 1,3 → pool 1 ; ids 2,4 → pool 2.
    repo.insert_batch(&[
        signal(1, Severity::Warning, base + Duration::seconds(1)),
        signal(2, Severity::Critical, base + Duration::seconds(2)),
        signal(1, Severity::Warning, base + Duration::seconds(3)),
        signal(2, Severity::Warning, base + Duration::seconds(4)),
    ])
    .await
    .unwrap();

    let page = repo
        .list(None, Some(pk(2)), None, PageDirection::Next, None, 10)
        .await
        .unwrap();

    assert_eq!(ids(&page), vec![4, 2]);
    assert!(page.items.iter().all(|r| r.signal.pool_address == pk(2)));
    assert!(page.is_first && page.is_last);

    // Combined with severity: AND semantics.
    let page = repo
        .list(
            Some(Severity::Critical),
            Some(pk(2)),
            None,
            PageDirection::Next,
            None,
            10,
        )
        .await
        .unwrap();
    assert_eq!(ids(&page), vec![2]);

    // A pool that never signalled yields an empty first/last page.
    let page = repo
        .list(None, Some(pk(9)), None, PageDirection::Next, None, 10)
        .await
        .unwrap();
    assert!(page.items.is_empty());
}

#[sqlx::test]
async fn severity_filter_restricts_the_feed(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    seed_five(&repo).await;

    let page = repo
        .list(
            Some(Severity::Critical),
            None,
            None,
            PageDirection::Next,
            None,
            10,
        )
        .await
        .unwrap();

    assert_eq!(ids(&page), vec![4, 2]);
    assert!(
        page.items
            .iter()
            .all(|r| r.signal.severity == Severity::Critical)
    );
    assert!(page.is_first && page.is_last);
}

// ── recent_by_pools — the pools-list signal indicator ────────────────

#[sqlx::test]
async fn recent_by_pools_groups_newest_first_and_windows(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    let now = Utc::now();

    // Pool 1: one stale signal (outside the window) and two recent.
    // Pool 2: one recent. Pool 3: quiet.
    repo.insert_batch(&[
        signal(1, Severity::Info, now - Duration::hours(30)), // id 1 — stale
        signal(1, Severity::Warning, now - Duration::hours(2)), // id 2
        signal(2, Severity::Critical, now - Duration::hours(1)), // id 3
        signal(1, Severity::Info, now - Duration::minutes(5)), // id 4
    ])
    .await
    .unwrap();

    let map = repo
        .recent_by_pools(&[pk(1), pk(2), pk(3)], now - Duration::hours(24), 20)
        .await
        .unwrap();

    // Pool 1: the two in-window signals, newest first; the stale one
    // filtered out. Pool 2: its single signal. Pool 3: absent.
    let pool1: Vec<i64> = map[&pk(1)].iter().map(|r| r.id).collect();
    assert_eq!(pool1, vec![4, 2]);
    let pool2: Vec<i64> = map[&pk(2)].iter().map(|r| r.id).collect();
    assert_eq!(pool2, vec![3]);
    assert!(!map.contains_key(&pk(3)));
    assert_eq!(map.len(), 2);
}

#[sqlx::test]
async fn recent_by_pools_caps_per_pool_not_globally(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    let now = Utc::now();

    // Pool 1 is noisy (3 signals), pool 2 has one: with a per-pool cap
    // of 2, pool 1 keeps its 2 newest and pool 2 is untouched.
    repo.insert_batch(&[
        signal(1, Severity::Info, now - Duration::hours(3)), // id 1
        signal(1, Severity::Warning, now - Duration::hours(2)), // id 2
        signal(1, Severity::Critical, now - Duration::hours(1)), // id 3
        signal(2, Severity::Info, now - Duration::hours(4)), // id 4
    ])
    .await
    .unwrap();

    let map = repo
        .recent_by_pools(&[pk(1), pk(2)], now - Duration::hours(24), 2)
        .await
        .unwrap();

    let pool1: Vec<i64> = map[&pk(1)].iter().map(|r| r.id).collect();
    assert_eq!(pool1, vec![3, 2]);
    let pool2: Vec<i64> = map[&pk(2)].iter().map(|r| r.id).collect();
    assert_eq!(pool2, vec![4]);
}

#[sqlx::test]
async fn recent_by_pools_empty_input_is_a_no_op(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);

    let map = repo
        .recent_by_pools(&[], Utc::now() - Duration::hours(24), 20)
        .await
        .unwrap();

    assert!(map.is_empty());
}
