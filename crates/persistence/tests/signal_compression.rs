//! A compressed chunk of `signals` serves a page filtered by protocol or by severity through
//! the Bloom filter TimescaleDB draws from each index, whether it was compressed before 040
//! or after. ⚠️ `compress_chunk` is called by hand: the local Postgres has no background
//! workers, so the policy never fires and a test relying on it would read uncompressed rows.

use super::helpers::pk;
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};

use yog_core::{
    PageDirection,
    domain::{Protocol, Severity, Signal, SignalFeed, SignalFilter, SignalRepository},
};
use yog_persistence::PgSignalRepository;

/// The filtered columns of `list`, and those a compressed chunk carries a single-column Bloom
/// filter on, before 040 and after; the filters TimescaleDB draws besides are not 040's.
const FILTERED: &[&str] = &["pool_address", "protocol", "severity"];
const BEFORE_040: &[&str] = &["pool_address"];
const AFTER_040: &[&str] = &["pool_address", "protocol", "severity"];

fn signal(protocol: Protocol, severity: Severity, pool: u8, days_ago: i64) -> Signal {
    Signal {
        detector: "flow_imbalance".to_string(),
        protocol,
        pool_address: pk(pool),
        severity,
        value: Decimal::new(75, 2),
        threshold: Some(Decimal::new(6, 1)),
        message: None,
        triggered_at: Utc::now() - Duration::days(days_ago) - Duration::minutes(pool.into()),
    }
}

/// Signals over four 7-day chunks: the common ones in each, the rare ones — one DLMM, one
/// `info`, one pool — in old chunks only, so each filter has a match to find once compressed.
async fn seed(pool: &PgPool) {
    let mut batch = Vec::new();
    for days_ago in [1, 8, 15, 22] {
        for pool in 1..=5 {
            batch.push(signal(
                Protocol::MeteoraDammV2,
                Severity::Warning,
                pool,
                days_ago,
            ));
        }
        batch.push(signal(
            Protocol::MeteoraDammV2,
            Severity::Critical,
            6,
            days_ago,
        ));
    }
    batch.push(signal(Protocol::MeteoraDlmm, Severity::Warning, 7, 22));
    batch.push(signal(Protocol::MeteoraDammV2, Severity::Info, 8, 15));
    PgSignalRepository::new(pool.clone())
        .insert_batch(&batch)
        .await
        .unwrap();
}

/// Compresses every chunk, the open one included, and asserts there are several.
async fn compress_every_chunk(pool: &PgPool) {
    let compressed: Vec<String> =
        sqlx::query_scalar("SELECT compress_chunk(c)::TEXT FROM show_chunks('signals') c")
            .fetch_all(pool)
            .await
            .expect("compressing the seeded chunks must succeed");
    assert!(
        compressed.len() >= 4,
        "the fixture must span several 7-day chunks, got {compressed:?} — otherwise the \
         compressed path is barely exercised"
    );
}

/// Per chunk of `signals`: compressed or not, and the Bloom filters its compressed form
/// carries, a filter over several columns written `a+b`. ⚠️ Read from TimescaleDB's own
/// catalog: no public view lists them.
async fn bloom_columns(pool: &PgPool) -> Vec<(bool, Vec<String>)> {
    sqlx::query(
        r"SELECT c.is_compressed AS compressed,
                 coalesce((SELECT array_agg(b.col ORDER BY b.col)
                             FROM (SELECT CASE jsonb_typeof(e->'column')
                                              WHEN 'array' THEN (SELECT string_agg(x, '+')
                                                                   FROM jsonb_array_elements_text(e->'column') x)
                                              ELSE e->>'column'
                                          END AS col
                                     FROM jsonb_array_elements(cs.index) e
                                    WHERE e->>'type' = 'bloom') b), '{}')::TEXT[] AS columns
            FROM timescaledb_information.chunks c
       LEFT JOIN _timescaledb_catalog.compression_settings cs
              ON cs.relid = format('%I.%I', c.chunk_schema, c.chunk_name)::REGCLASS
           WHERE c.hypertable_name = 'signals'
           ORDER BY c.range_start",
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .iter()
    .map(|row| (row.get("compressed"), row.get("columns")))
    .collect()
}

fn every_chunk_compressed_with(chunks: &[(bool, Vec<String>)], expected: &[&str]) -> bool {
    !chunks.is_empty()
        && chunks.iter().all(|(compressed, columns)| {
            let filtered: Vec<&str> = FILTERED
                .iter()
                .copied()
                .filter(|column| columns.iter().any(|c| c == column))
                .collect();
            *compressed && filtered == expected
        })
}

/// The five filtered pages, as ids, newest first.
async fn filtered_pages(pool: &PgPool) -> Vec<(&'static str, Vec<i64>)> {
    let repo = PgSignalRepository::new(pool.clone());
    let filters = [
        (
            "meteora_damm_v2",
            SignalFilter {
                protocol: Some(Protocol::MeteoraDammV2),
                ..Default::default()
            },
        ),
        (
            "meteora_dlmm",
            SignalFilter {
                protocol: Some(Protocol::MeteoraDlmm),
                ..Default::default()
            },
        ),
        (
            "info",
            SignalFilter {
                severity: Some(Severity::Info),
                ..Default::default()
            },
        ),
        (
            "critical",
            SignalFilter {
                severity: Some(Severity::Critical),
                ..Default::default()
            },
        ),
        (
            "pool 8",
            SignalFilter {
                pool: Some(pk(8)),
                ..Default::default()
            },
        ),
    ];
    let mut pages = Vec::new();
    for (name, filter) in filters {
        let page = repo
            .list(filter, None, PageDirection::Next, None, 50)
            .await
            .unwrap();
        pages.push((name, page.items.iter().map(|r| r.id).collect()));
    }
    pages
}

/// Against 040 without its decompress-and-compress loop, or without either index.
#[sqlx::test(migrations = false)]
async fn a_chunk_compressed_before_040_gains_both_filters(pool: PgPool) {
    let migrator = sqlx::migrate!("./migrations");
    migrator.run_to(39, &pool).await.unwrap();
    seed(&pool).await;
    compress_every_chunk(&pool).await;

    let before = bloom_columns(&pool).await;
    assert!(
        every_chunk_compressed_with(&before, BEFORE_040),
        "before 040 every chunk must be compressed with a Bloom filter on pool_address alone \
         of the filtered columns, or what follows does not show 040 adding the two: {before:?}"
    );

    migrator.run_to(40, &pool).await.unwrap();

    let after = bloom_columns(&pool).await;
    assert!(
        every_chunk_compressed_with(&after, AFTER_040),
        "after 040 a chunk compressed before it must still be compressed and carry a Bloom \
         filter on protocol and on severity, drawn from the two new indexes: {after:?}"
    );
}

/// Against 040 without either index, or `timescaledb.auto_sparse_indexes` turned off.
#[sqlx::test]
async fn a_chunk_compressed_after_040_carries_both_filters(pool: PgPool) {
    seed(&pool).await;
    compress_every_chunk(&pool).await;

    let chunks = bloom_columns(&pool).await;
    assert!(
        every_chunk_compressed_with(&chunks, AFTER_040),
        "a chunk compressed after 040 must carry a Bloom filter on each filtered column: \
         {chunks:?}"
    );
}

/// Against a compressed chunk that loses a match: TimescaleDB's behaviour, not 040's.
#[sqlx::test]
async fn a_compressed_chunk_serves_the_same_filtered_pages(pool: PgPool) {
    seed(&pool).await;

    let before = filtered_pages(&pool).await;
    let lengths: Vec<(&str, usize)> = before.iter().map(|(n, ids)| (*n, ids.len())).collect();
    assert_eq!(
        lengths,
        vec![
            ("meteora_damm_v2", 25),
            ("meteora_dlmm", 1),
            ("info", 1),
            ("critical", 4),
            ("pool 8", 1)
        ],
        "the fixture must give each filter its matches before anything is compressed, or the \
         comparison below compares two absences"
    );

    compress_every_chunk(&pool).await;

    assert_eq!(
        filtered_pages(&pool).await,
        before,
        "a Bloom filter may skip a compressed batch only when its value is absent: a match lost \
         to compression is a signal no filtered page shows"
    );
}
