//! Integration test for the paginated signal feed read
//! (`PgSignalRepository::list`).
//!
//! Gated behind `integration-tests`. Validates the vertical slice on the
//! real `signals` hypertable: display order (`triggered_at DESC, id DESC`),
//! forward/backward keyset navigation, boundary flags, the id tie-break on
//! equal timestamps, and the optional severity filter.

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::{
    Cursor, PageDirection, PagePosition,
    domain::{Protocol, Severity, Signal, SignalCursor, SignalFeedRepository, SignalRepository},
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

/// Seed five signals at strictly increasing timestamps. In a fresh test
/// database the BIGSERIAL assigns ids 1..=5 in insert order, so id N was
/// triggered at `base + N seconds` — newest first is 5,4,3,2,1.
async fn seed_five(repo: &PgSignalRepository) {
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
}

#[sqlx::test]
async fn feed_paginates_newest_first(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    seed_five(&repo).await;

    // Page 1: newest two.
    let page1 = repo
        .list(None, None, PageDirection::Next, None, 2)
        .await
        .unwrap();
    assert_eq!(ids(&page1), vec![5, 4]);
    assert!(page1.is_first && !page1.is_last);
    assert!(page1.prev_cursor.is_none());

    // Page 2 via the forward cursor.
    let cursor = signal_cursor(page1.next_cursor.as_ref().unwrap());
    let page2 = repo
        .list(None, Some(cursor), PageDirection::Next, None, 2)
        .await
        .unwrap();
    assert_eq!(ids(&page2), vec![3, 2]);
    assert!(!page2.is_first && !page2.is_last);

    // Page 3: the tail.
    let cursor = signal_cursor(page2.next_cursor.as_ref().unwrap());
    let page3 = repo
        .list(None, Some(cursor), PageDirection::Next, None, 2)
        .await
        .unwrap();
    assert_eq!(ids(&page3), vec![1]);
    assert!(!page3.is_first && page3.is_last);
    assert!(page3.next_cursor.is_none());

    // Backward from page 2 returns page 1, in display order.
    let cursor = signal_cursor(page2.prev_cursor.as_ref().unwrap());
    let back = repo
        .list(None, Some(cursor), PageDirection::Prev, None, 2)
        .await
        .unwrap();
    assert_eq!(ids(&back), vec![5, 4]);
    assert!(back.is_first);

    // Position jumps ignore cursors: Last = the oldest page.
    let last = repo
        .list(None, None, PageDirection::Next, Some(PagePosition::Last), 2)
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
        .list(None, None, PageDirection::Next, None, 1)
        .await
        .unwrap();
    assert_eq!(ids(&page1), vec![2]);

    // The cursor must step past the tie deterministically, not skip or
    // repeat the sibling row.
    let cursor = signal_cursor(page1.next_cursor.as_ref().unwrap());
    let page2 = repo
        .list(None, Some(cursor), PageDirection::Next, None, 1)
        .await
        .unwrap();
    assert_eq!(ids(&page2), vec![1]);
    assert!(page2.is_last);
}

#[sqlx::test]
async fn severity_filter_restricts_the_feed(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    seed_five(&repo).await;

    let page = repo
        .list(
            Some(Severity::Critical),
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
