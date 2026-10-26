//! `PgAnnouncementRepository::list_entries` under `yog_api`: every announcement with where its
//! window stands at an instant, the latest start first, filtered by that status, paged without
//! repeat nor gap through shared starts.

use chrono::{DateTime, Duration, TimeZone, Utc};
use sqlx::PgPool;
use yog_core::{
    Cursor,
    domain::{AnnouncementLookup, AnnouncementStatus},
};
use yog_persistence::PgAnnouncementRepository;

use super::helpers::setup_roles_and_connect_as;

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::seconds(seconds)
}

async fn insert(pool: &PgPool, starts: i64, ends: Option<i64>) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO announcements (kind, severity, message, starts_at, ends_at)
         VALUES ('maintenance', 'info', 'm', $1, $2) RETURNING id",
    )
    .bind(at(starts))
    .bind(ends.map(at))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn read_all(
    repository: &PgAnnouncementRepository,
    status: Option<AnnouncementStatus>,
    limit: i64,
) -> Vec<(i64, AnnouncementStatus)> {
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = repository
            .list_entries(status, at(0), cursor.take(), limit)
            .await
            .unwrap();
        seen.extend(page.items.iter().map(|e| (e.announcement.id, e.status)));
        // ⚠️ A cursor that rereads its own row would page forever: fail instead of hanging.
        assert!(seen.len() <= 100, "paging does not end: {seen:?}");
        match page.next_cursor {
            Some(Cursor::Announcement(next)) => cursor = Some(*next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }
    seen
}

/// Mutation: a bound of the window moved (`>=` for `>`, `<` for `<=`) or the order reversed —
/// an announcement starting or ending at the instant under another status, or the list in
/// another order.
#[sqlx::test(migrations = "./migrations")]
async fn each_announcement_says_where_its_window_stands(pool: PgPool) {
    let open_for_good = insert(&pool, -100, None).await;
    let ends_now = insert(&pool, -100, Some(0)).await;
    let starts_later = insert(&pool, 100, None).await;
    let ended = insert(&pool, -200, Some(-50)).await;
    let open_until_later = insert(&pool, -100, Some(50)).await;
    let starts_now = insert(&pool, 0, None).await;
    let repository =
        PgAnnouncementRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let all = read_all(&repository, None, 50).await;

    use AnnouncementStatus::{Closed, Open, Upcoming};
    // The latest start first, then the highest id: three share the start at -100.
    assert_eq!(
        all,
        [
            (starts_later, Upcoming),
            (starts_now, Open),
            (open_until_later, Open),
            (ends_now, Closed),
            (open_for_good, Open),
            (ended, Closed),
        ]
    );
}

/// Mutation: the status filter dropped, or the cursor compared with `<=` — another status
/// listed, or an announcement sharing a start repeated or skipped.
#[sqlx::test(migrations = "./migrations")]
async fn a_status_and_its_pages_read_only_their_announcements(pool: PgPool) {
    for (starts, ends) in [
        (-100, None),
        (-100, Some(0)),
        (100, None),
        (-200, Some(-50)),
        (-100, Some(50)),
        (0, None),
    ] {
        insert(&pool, starts, ends).await;
    }
    let repository =
        PgAnnouncementRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let all = read_all(&repository, None, 50).await;
    let by_one = read_all(&repository, None, 1).await;
    let open = read_all(&repository, Some(AnnouncementStatus::Open), 1).await;
    let closed = read_all(&repository, Some(AnnouncementStatus::Closed), 50).await;
    let upcoming = read_all(&repository, Some(AnnouncementStatus::Upcoming), 50).await;

    assert_eq!(by_one, all);
    let only = |status| -> Vec<_> { all.iter().copied().filter(|e| e.1 == status).collect() };
    assert_eq!(open, only(AnnouncementStatus::Open));
    assert_eq!(closed, only(AnnouncementStatus::Closed));
    assert_eq!(upcoming, only(AnnouncementStatus::Upcoming));
    assert_eq!((open.len(), closed.len(), upcoming.len()), (3, 2, 1));
}
