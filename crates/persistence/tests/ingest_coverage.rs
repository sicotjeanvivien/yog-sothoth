//! `PgIngestCoverageRepository` under `yog_indexer`, and the gaps `ingest_coverage_gaps` reads
//! from the ranges it writes.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use yog_core::domain::{IngestCoverageRepository, ObservedSlots, Protocol, SlotRange};
use yog_persistence::PgIngestCoverageRepository;

use super::helpers::{setup_roles_and_connect_as, ts};

const DAMM_V2: Protocol = Protocol::MeteoraDammV2;
const DLMM: Protocol = Protocol::MeteoraDlmm;

async fn repository(pool: &PgPool) -> PgIngestCoverageRepository {
    PgIngestCoverageRepository::new(setup_roles_and_connect_as(pool, "yog_indexer").await)
}

/// The block time of a slot, one second per slot from `ts()`.
fn time_of(slot: u64) -> DateTime<Utc> {
    ts() + Duration::seconds(slot as i64)
}

fn range(from: u64, to: u64) -> SlotRange {
    let mut range = SlotRange::starting(from, time_of(from));
    range.include(to, time_of(to));
    range
}

/// One session, keyed by `session` seconds after `ts()`.
fn observed(protocol: Protocol, session: i64, from: u64, to: u64) -> ObservedSlots {
    ObservedSlots {
        protocol,
        session_started_at: ts() + Duration::seconds(session),
        range: range(from, to),
    }
}

type Row = (i64, i64, DateTime<Utc>, DateTime<Utc>);

async fn rows(pool: &PgPool) -> Vec<Row> {
    sqlx::query_as(
        "SELECT observed_from, observed_to, observed_from_at, observed_to_at
           FROM ingest_coverage ORDER BY protocol, session_started_at",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

type Gap = (String, i64, i64, DateTime<Utc>, DateTime<Utc>);

async fn gaps(pool: &PgPool) -> Vec<Gap> {
    sqlx::query_as(
        "SELECT protocol, missing_from, missing_to, last_seen_at, seen_again_at
           FROM ingest_coverage_gaps ORDER BY protocol, missing_from",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Mutation: an end overwritten by the value received, `GREATEST` or `LEAST` removed.
#[sqlx::test(migrations = "./migrations")]
async fn a_range_reads_back_and_only_ever_widens(pool: PgPool) {
    let repo = repository(&pool).await;

    repo.record(&observed(DAMM_V2, 0, 100, 200)).await.unwrap();
    assert_eq!(rows(&pool).await, [(100, 200, time_of(100), time_of(200))]);

    repo.record(&observed(DAMM_V2, 0, 90, 250)).await.unwrap();
    assert_eq!(rows(&pool).await, [(90, 250, time_of(90), time_of(250))]);

    repo.record(&observed(DAMM_V2, 0, 120, 180)).await.unwrap();
    assert_eq!(
        rows(&pool).await,
        [(90, 250, time_of(90), time_of(250))],
        "a narrower write leaves the row as it is"
    );
}

/// Mutation: the `+ 1` or the `- 1` of the view removed.
#[sqlx::test(migrations = "./migrations")]
async fn a_cut_between_two_sessions_is_one_gap_with_its_times(pool: PgPool) {
    let repo = repository(&pool).await;
    repo.record(&observed(DAMM_V2, 0, 100, 200)).await.unwrap();
    repo.record(&observed(DAMM_V2, 600, 260, 300))
        .await
        .unwrap();

    assert_eq!(
        gaps(&pool).await,
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
async fn overlapping_nested_and_touching_ranges_leave_no_gap(pool: PgPool) {
    let repo = repository(&pool).await;
    // A replay overlapping the session before, a session inside the first, one touching the end.
    repo.record(&observed(DAMM_V2, 0, 100, 400)).await.unwrap();
    repo.record(&observed(DAMM_V2, 60, 150, 200)).await.unwrap();
    repo.record(&observed(DAMM_V2, 120, 380, 500))
        .await
        .unwrap();
    repo.record(&observed(DAMM_V2, 180, 501, 600))
        .await
        .unwrap();

    assert_eq!(gaps(&pool).await, []);
}

/// Mutation: the view's `PARTITION BY protocol` removed.
#[sqlx::test(migrations = "./migrations")]
async fn one_protocol_ranges_do_not_close_or_open_another_protocol_gaps(pool: PgPool) {
    let repo = repository(&pool).await;
    repo.record(&observed(DAMM_V2, 0, 100, 200)).await.unwrap();
    repo.record(&observed(DLMM, 0, 150, 300)).await.unwrap();
    repo.record(&observed(DAMM_V2, 600, 260, 300))
        .await
        .unwrap();

    let found: Vec<(String, i64, i64)> = gaps(&pool)
        .await
        .into_iter()
        .map(|(protocol, from, to, _, _)| (protocol, from, to))
        .collect();
    assert_eq!(found, [("meteora_damm_v2".to_string(), 201, 259)]);
}
