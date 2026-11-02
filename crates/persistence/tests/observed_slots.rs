//! `PgObservedSlotsRepository` under `yog_indexer`, and the ranges nobody watched that
//! `unobserved_slot_ranges` reads from what it writes.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use yog_core::domain::{ObservedSlots, ObservedSlotsRepository, Protocol};
use yog_persistence::PgObservedSlotsRepository;

use super::helpers::{setup_roles_and_connect_as, ts};

const DAMM_V2: Protocol = Protocol::MeteoraDammV2;
const DLMM: Protocol = Protocol::MeteoraDlmm;

async fn repository(pool: &PgPool) -> PgObservedSlotsRepository {
    PgObservedSlotsRepository::new(setup_roles_and_connect_as(pool, "yog_indexer").await)
}

/// The block time of a slot, one second per slot from `ts()`.
fn time_of(slot: u64) -> DateTime<Utc> {
    ts() + Duration::seconds(slot as i64)
}

/// What one session watched, keyed by `session` seconds after `ts()`.
fn watched(protocol: Protocol, session: i64, first_slot: u64, last_slot: u64) -> ObservedSlots {
    ObservedSlots {
        protocol,
        session_started_at: ts() + Duration::seconds(session),
        first_slot,
        first_slot_at: time_of(first_slot),
        last_slot,
        last_slot_at: time_of(last_slot),
    }
}

type Row = (i64, i64, DateTime<Utc>, DateTime<Utc>);

async fn rows(pool: &PgPool) -> Vec<Row> {
    sqlx::query_as(
        "SELECT first_slot, last_slot, first_slot_at, last_slot_at
           FROM observed_slot_ranges ORDER BY protocol, session_started_at",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

type Unwatched = (String, i64, i64, DateTime<Utc>, DateTime<Utc>);

async fn unwatched(pool: &PgPool) -> Vec<Unwatched> {
    sqlx::query_as(
        "SELECT protocol, first_slot, last_slot, last_seen_at, seen_again_at
           FROM unobserved_slot_ranges ORDER BY protocol, first_slot",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Mutation: an end overwritten by the value received, `GREATEST` or `LEAST` removed.
#[sqlx::test(migrations = "./migrations")]
async fn a_range_reads_back_and_only_ever_widens(pool: PgPool) {
    let repo = repository(&pool).await;

    repo.record(&watched(DAMM_V2, 0, 100, 200)).await.unwrap();
    assert_eq!(rows(&pool).await, [(100, 200, time_of(100), time_of(200))]);

    repo.record(&watched(DAMM_V2, 0, 90, 250)).await.unwrap();
    assert_eq!(rows(&pool).await, [(90, 250, time_of(90), time_of(250))]);

    repo.record(&watched(DAMM_V2, 0, 120, 180)).await.unwrap();
    assert_eq!(
        rows(&pool).await,
        [(90, 250, time_of(90), time_of(250))],
        "a narrower write leaves the row as it is"
    );
}

/// Mutation: the `+ 1` or the `- 1` of the view removed.
#[sqlx::test(migrations = "./migrations")]
async fn a_cut_between_two_sessions_is_one_unwatched_range_with_its_times(pool: PgPool) {
    let repo = repository(&pool).await;
    repo.record(&watched(DAMM_V2, 0, 100, 200)).await.unwrap();
    repo.record(&watched(DAMM_V2, 600, 260, 300)).await.unwrap();

    assert_eq!(
        unwatched(&pool).await,
        [(
            "meteora_damm_v2".to_string(),
            201,
            259,
            time_of(200),
            time_of(260)
        )]
    );
}

/// Mutation: each range compared to the previous one only (`LAG`), not to the highest before.
#[sqlx::test(migrations = "./migrations")]
async fn overlapping_nested_and_touching_ranges_leave_nothing_unwatched(pool: PgPool) {
    let repo = repository(&pool).await;
    // A replay overlapping the session before, a session inside the first, one touching the end.
    repo.record(&watched(DAMM_V2, 0, 100, 400)).await.unwrap();
    repo.record(&watched(DAMM_V2, 60, 150, 200)).await.unwrap();
    repo.record(&watched(DAMM_V2, 120, 380, 500)).await.unwrap();
    repo.record(&watched(DAMM_V2, 180, 501, 600)).await.unwrap();

    assert_eq!(unwatched(&pool).await, []);
}

/// Mutation: the view's `PARTITION BY protocol` removed.
#[sqlx::test(migrations = "./migrations")]
async fn one_protocol_does_not_watch_for_another(pool: PgPool) {
    let repo = repository(&pool).await;
    repo.record(&watched(DAMM_V2, 0, 100, 200)).await.unwrap();
    repo.record(&watched(DLMM, 0, 150, 300)).await.unwrap();
    repo.record(&watched(DAMM_V2, 600, 260, 300)).await.unwrap();

    let found: Vec<(String, i64, i64)> = unwatched(&pool)
        .await
        .into_iter()
        .map(|(protocol, first, last, _, _)| (protocol, first, last))
        .collect();
    assert_eq!(found, [("meteora_damm_v2".to_string(), 201, 259)]);
}
