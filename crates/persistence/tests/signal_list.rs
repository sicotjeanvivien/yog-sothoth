//! `PgSignalRepository::list` on the real `signals` hypertable: display order, keyset
//! navigation, the id tie-break, and the filters.

use super::helpers::{DAMM, DLMM, as_of, pk, setup_roles_and_connect_as};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use yog_core::{
    Cursor, PageDirection, PagePosition,
    domain::{
        Protocol, Severity, Signal, SignalCursor, SignalFeed, SignalFilter, SignalRepository,
    },
};
use yog_persistence::PgSignalRepository;

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
    let Cursor::Signal(c) = cursor else {
        panic!("a signal page yields signal cursors: {cursor:?}")
    };
    c.clone()
}

/// Five signals at increasing timestamps; ids 1..=5 in insert order, so newest first is 5..1.
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
        .list(SignalFilter::default(), None, PageDirection::Next, None, 2)
        .await
        .unwrap();
    assert_eq!(ids(&page1), vec![5, 4]);
    assert!(page1.is_first && !page1.is_last);
    assert!(page1.prev_cursor.is_none());

    // Page 2 via the forward cursor.
    let cursor = signal_cursor(page1.next_cursor.as_ref().unwrap());
    let page2 = repo
        .list(
            SignalFilter::default(),
            Some(cursor),
            PageDirection::Next,
            None,
            2,
        )
        .await
        .unwrap();
    assert_eq!(ids(&page2), vec![3, 2]);
    assert!(!page2.is_first && !page2.is_last);

    // Page 3: the tail.
    let cursor = signal_cursor(page2.next_cursor.as_ref().unwrap());
    let page3 = repo
        .list(
            SignalFilter::default(),
            Some(cursor),
            PageDirection::Next,
            None,
            2,
        )
        .await
        .unwrap();
    assert_eq!(ids(&page3), vec![1]);
    assert!(!page3.is_first && page3.is_last);
    assert!(page3.next_cursor.is_none());

    // Backward from page 2 returns page 1, in display order.
    let cursor = signal_cursor(page2.prev_cursor.as_ref().unwrap());
    let back = repo
        .list(
            SignalFilter::default(),
            Some(cursor),
            PageDirection::Prev,
            None,
            2,
        )
        .await
        .unwrap();
    assert_eq!(ids(&back), vec![5, 4]);
    assert!(back.is_first);

    // Position jumps ignore cursors: Last = the oldest page.
    let last = repo
        .list(
            SignalFilter::default(),
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
        .list(SignalFilter::default(), None, PageDirection::Next, None, 1)
        .await
        .unwrap();
    assert_eq!(ids(&page1), vec![2]);

    // The cursor must step past the tie deterministically, not skip or
    // repeat the sibling row.
    let cursor = signal_cursor(page1.next_cursor.as_ref().unwrap());
    let page2 = repo
        .list(
            SignalFilter::default(),
            Some(cursor),
            PageDirection::Next,
            None,
            1,
        )
        .await
        .unwrap();
    assert_eq!(ids(&page2), vec![1]);
    assert!(page2.is_last);

    let cursor = signal_cursor(page2.prev_cursor.as_ref().unwrap());
    let back = repo
        .list(
            SignalFilter::default(),
            Some(cursor),
            PageDirection::Prev,
            None,
            1,
        )
        .await
        .unwrap();
    assert_eq!(ids(&back), vec![2]);
}

/// Mutation: the live lens keyed on `triggered_at` again — the signal stamped earlier but
/// inserted later is skipped.
#[sqlx::test]
async fn the_live_lens_follows_the_ids_not_the_timestamps(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    let day = Duration::days(1);
    assert_eq!(repo.live_tip(day).await.unwrap(), None);

    let now = Utc::now();
    repo.insert_batch(&[signal(1, Severity::Warning, now - Duration::minutes(5))])
        .await
        .unwrap();
    repo.insert_batch(&[signal(2, Severity::Warning, now - Duration::minutes(30))])
        .await
        .unwrap();

    assert_eq!(repo.live_tip(day).await.unwrap(), Some(2));
    let after_one = repo.live_after(1, day, 10).await.unwrap();
    assert_eq!(after_one.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2]);
    assert!(repo.live_after(2, day, 10).await.unwrap().is_empty());
}

/// Mutation: the lookback dropped from either query — the old signal becomes the tip, or comes.
#[sqlx::test]
async fn the_live_lens_reads_only_the_lookback(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    let now = Utc::now();
    repo.insert_batch(&[
        signal(1, Severity::Warning, now - Duration::minutes(5)),
        signal(1, Severity::Warning, now - Duration::days(2)),
    ])
    .await
    .unwrap();

    let day = Duration::days(1);
    assert_eq!(repo.live_tip(day).await.unwrap(), Some(1));
    let all = repo.live_after(0, day, 10).await.unwrap();
    assert_eq!(all.iter().map(|r| r.id).collect::<Vec<_>>(), vec![1]);
}

/// Mutation: the limit dropped — the whole tail comes in one batch.
#[sqlx::test]
async fn the_live_lens_caps_its_batch(pool: PgPool) {
    let repo = PgSignalRepository::new(pool);
    seed_five(&repo).await;

    let capped = repo.live_after(2, Duration::days(1), 2).await.unwrap();
    assert_eq!(capped.iter().map(|r| r.id).collect::<Vec<_>>(), vec![3, 4]);
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
        .list(
            SignalFilter {
                pool: Some(pk(2)),
                ..SignalFilter::default()
            },
            None,
            PageDirection::Next,
            None,
            10,
        )
        .await
        .unwrap();

    assert_eq!(ids(&page), vec![4, 2]);
    assert!(page.items.iter().all(|r| r.signal.pool_address == pk(2)));
    assert!(page.is_first && page.is_last);

    // Combined with severity: AND semantics.
    let page = repo
        .list(
            SignalFilter {
                severity: Some(Severity::Critical),
                pool: Some(pk(2)),
                ..SignalFilter::default()
            },
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
        .list(
            SignalFilter {
                pool: Some(pk(9)),
                ..SignalFilter::default()
            },
            None,
            PageDirection::Next,
            None,
            10,
        )
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
            SignalFilter {
                severity: Some(Severity::Critical),
                ..SignalFilter::default()
            },
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

/// Mutation: the protocol filter of the feed dropped — DLMM's signal listed.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_signals_are_its_own(pool: PgPool) {
    for protocol in [DAMM, DLMM] {
        sqlx::query(
            "INSERT INTO signals
               (detector, protocol, pool_address, severity, value, threshold, message,
                triggered_at)
             VALUES ('flow_imbalance', $1, $2, 'warning', 1, 1, NULL, $3)",
        )
        .bind(protocol)
        .bind(pk(1).to_string())
        .bind(as_of())
        .execute(&pool)
        .await
        .unwrap();
    }
    let repository = PgSignalRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let page = repository
        .list(
            SignalFilter {
                protocol: Some(Protocol::MeteoraDammV2),
                ..SignalFilter::default()
            },
            None,
            PageDirection::Next,
            None,
            50,
        )
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].signal.protocol, Protocol::MeteoraDammV2);
}
