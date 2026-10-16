//! The price series is compressed past seven days, and compressing it changes nothing a
//! reader sees: the catalog's policy, both lookup shapes over compressed chunks, and writes
//! into one. ⚠️ `compress_chunk` is called by hand: the local Postgres has no background
//! workers, so the policy never fires and a test relying on it would read uncompressed rows.

use super::helpers::{UNIQUE_VIOLATION, pk, price_mint_since, record_prices, sqlstate};
use std::str::FromStr;

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};

use yog_core::domain::{PriceProvider, TokenPrice, TokenPriceLookup, TokenPriceRepository};
use yog_persistence::PgTokenPriceRepository;

/// Days of hourly prices seeded: several 7-day chunks, so the unbounded latest lookup
/// reads a compressed one.
const SEEDED_DAYS: i64 = 30;

/// One row of `timescaledb_information.compression_settings`: a column is a segment key or
/// part of the order inside a segment, so exactly one index is set.
#[derive(Debug, PartialEq, Eq)]
struct CompressionSetting {
    column: String,
    segmentby_index: Option<i16>,
    orderby_index: Option<i16>,
    /// `Some(false)` for a `DESC` ordering column, `None` on a segment key.
    orderby_asc: Option<bool>,
}

#[sqlx::test]
async fn the_compression_policy_is_declared_as_intended(pool: PgPool) {
    // ⚠️ Compared as an INTERVAL: the catalog's text follows the `IntervalStyle` of creation.
    let policies: Vec<(bool, String)> = sqlx::query(
        "SELECT (config->>'compress_after')::interval = INTERVAL '7 days' AS is_house_delay,
                (config->>'compress_after')                               AS declared
           FROM timescaledb_information.jobs
          WHERE proc_name = 'policy_compression'
            AND hypertable_name = 'token_prices'",
    )
    .fetch_all(&pool)
    .await
    .expect("the compression policy must be readable from the catalog")
    .iter()
    .map(|row| (row.get("is_house_delay"), row.get("declared")))
    .collect();

    assert_eq!(
        policies.len(),
        1,
        "token_prices must carry exactly one compression policy, got {policies:?}"
    );
    assert!(
        policies[0].0,
        "the compression delay must be the house 7 days — the one every other \
         hypertable `001` compresses uses, `signals` excepted at 30 days and \
         the swaps at 1 day since 014 — and the catalog declares {}",
        policies[0].1
    );

    // No retention, on purpose: an as-of gap never heals, nothing backfills prices.
    let retention: i64 = sqlx::query_scalar(
        "SELECT count(*)::BIGINT
           FROM timescaledb_information.jobs
          WHERE proc_name = 'policy_retention'
            AND hypertable_name = 'token_prices'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        retention, 0,
        "token_prices must have NO retention policy: the valuation views read \
         the price history bucket by bucket, and a dropped price is a \
         permanently unvalued bucket. If you are adding one, that is a decision \
         to write down, not a test to update"
    );

    let settings: Vec<CompressionSetting> = sqlx::query(
        "SELECT attname::TEXT, segmentby_column_index, orderby_column_index, orderby_asc
           FROM timescaledb_information.compression_settings
          WHERE hypertable_name = 'token_prices'
          ORDER BY attname",
    )
    .fetch_all(&pool)
    .await
    .unwrap()
    .iter()
    .map(|row| CompressionSetting {
        column: row.get("attname"),
        segmentby_index: row.get("segmentby_column_index"),
        orderby_index: row.get("orderby_column_index"),
        orderby_asc: row.get("orderby_asc"),
    })
    .collect();

    assert_eq!(
        settings,
        vec![
            CompressionSetting {
                column: "fetched_at".to_string(),
                segmentby_index: None,
                orderby_index: Some(1),
                orderby_asc: Some(false),
            },
            CompressionSetting {
                column: "mint".to_string(),
                segmentby_index: Some(1),
                orderby_index: None,
                orderby_asc: None,
            },
        ],
        "compression must segment by mint and order by fetched_at DESC — every \
         reader asks for the most recent row of ONE mint, and that pair is the \
         primary key"
    );

    let indexes: Vec<String> = sqlx::query_scalar(
        "SELECT indexname::TEXT FROM pg_indexes
          WHERE tablename = 'token_prices' ORDER BY indexname",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        indexes,
        vec![
            "idx_token_prices_mint_recent".to_string(),
            "token_prices_pkey".to_string(),
        ],
        "`token_prices_fetched_at_idx` — 61 MB, put there by create_hypertable \
         and needed by no written query, none of which looks for a price at an \
         instant without naming its mint — must be gone, and the two written \
         indexes must both still be there"
    );
}

/// What the two lookup shapes return. NUMERIC is compared as text: a decimal type could
/// round both sides alike.
#[derive(Debug, PartialEq, Eq)]
struct Readings {
    /// The as-of shape, bounded to one hour before each bucket.
    buckets: i64,
    volume_usd: Option<String>,
    /// `find_latest_by_mint`, unbounded in time: the one that lands on a compressed chunk.
    latest_price: Option<String>,
    latest_fetched_at: Option<DateTime<Utc>>,
    latest_confidence: Option<String>,
    /// Every `confidence`, in key order: the only nullable `REAL` column, on its own codec path.
    confidence_digest: Option<String>,
}

async fn read_both_shapes(pool: &PgPool, pool_addr: &str) -> Readings {
    let activity = sqlx::query(
        "SELECT count(*)::BIGINT AS buckets, sum(volume_usd)::TEXT AS volume_usd
           FROM meteora_damm_v2_pool_hourly_activity
          WHERE pool_address = $1",
    )
    .bind(pool_addr)
    .fetch_one(pool)
    .await
    .unwrap();

    let latest = PgTokenPriceRepository::new(pool.clone())
        .find_latest_by_mint(&pk(2))
        .await
        .unwrap();

    let confidence_digest: Option<String> = sqlx::query_scalar(
        "SELECT md5(string_agg(coalesce(confidence::TEXT, '-'), '|'
                               ORDER BY mint, fetched_at))
           FROM token_prices",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    Readings {
        buckets: activity.get("buckets"),
        volume_usd: activity.get("volume_usd"),
        latest_price: latest.as_ref().map(|p| p.price_usd.to_string()),
        latest_fetched_at: latest.as_ref().map(|p| p.fetched_at),
        latest_confidence: latest.and_then(|p| p.confidence).map(|c| c.to_string()),
        confidence_digest,
    }
}

/// A pool with resolved mints and `SEEDED_DAYS` of hourly prices and swaps.
async fn seed(pool: &PgPool) -> String {
    let pool_addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(&pool_addr)
    .bind(&mint_a)
    .bind(&mint_b)
    .execute(pool)
    .await
    .unwrap();

    for (mint, decimals) in [(&mint_a, 6i16), (&mint_b, 9i16)] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1,$2,$3,$3)",
        )
        .bind(mint)
        .bind(decimals)
        .bind(now - Duration::days(SEEDED_DAYS))
        .execute(pool)
        .await
        .unwrap();
    }

    // ⚠️ A moving price, or the comparison cannot tell rows apart; `confidence` set on one
    // mint and NULL on the other, so both states survive the compressed form.
    for (mint, base, confidence) in [(pk(2), "1.5", Some(0.75_f32)), (pk(3), "100.0", None)] {
        let series: Vec<TokenPrice> = (0..=SEEDED_DAYS * 24)
            .map(|h| TokenPrice {
                mint,
                price_usd: Decimal::from_str(base).unwrap() + Decimal::new(h, 3),
                price_provider: PriceProvider::Jupiter,
                confidence,
                fetched_at: now - Duration::hours(h),
            })
            .collect();
        record_prices(pool, &series).await;
    }

    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         SELECT $1, 'sig-' || h, 'a_to_b', 1000000, 0, 0, 0, 0, 100, 10, 0, 0, true,
                now() - (h || ' hours')::interval, 0, 0
           FROM generate_series(1, $2::INT * 24) h",
    )
    .bind(&pool_addr)
    .bind(SEEDED_DAYS as i32)
    .execute(pool)
    .await
    .unwrap();

    pool_addr
}

/// Compresses every chunk, the open one included — the policy never would — so the latest
/// lookup reads compressed rows. Asserts they are.
async fn compress_every_chunk(pool: &PgPool) {
    sqlx::query("SELECT compress_chunk(c) FROM show_chunks('token_prices') c")
        .execute(pool)
        .await
        .expect("compressing the seeded chunks must succeed");

    let row = sqlx::query(
        "SELECT (count(*) FILTER (WHERE is_compressed))::BIGINT AS compressed,
                count(*)::BIGINT                                AS total
           FROM timescaledb_information.chunks
          WHERE hypertable_name = 'token_prices'",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let (compressed, total): (i64, i64) = (row.get("compressed"), row.get("total"));
    assert!(
        total >= 4,
        "the fixture must span several 7-day chunks, got {total} — otherwise \
         the compressed path is barely exercised"
    );
    assert_eq!(
        compressed, total,
        "every chunk must be compressed before the re-read, or the rest of this \
         test passes on an uncompressed table and proves nothing"
    );
}

#[sqlx::test]
async fn a_compressed_chunk_serves_the_same_prices(pool: PgPool) {
    let pool_addr = seed(&pool).await;

    let before = read_both_shapes(&pool, &pool_addr).await;
    assert!(
        before.volume_usd.is_some() && before.buckets > 0 && before.latest_confidence.is_some(),
        "the fixture must produce valued buckets and a non-NULL \
         confidence before anything is compressed, or the comparison below \
         compares two absences: {before:?}"
    );

    compress_every_chunk(&pool).await;

    let after = read_both_shapes(&pool, &pool_addr).await;
    assert_eq!(
        after, before,
        "compression must be invisible to both shapes of price lookup — the \
         latest one (find_latest_by_mint, no time bound) and the as-of one \
         (the hourly activity view, bounded to one hour before each bucket). \
         Baseline §7 refused compression for fear of exactly these lookups"
    );
}

async fn count_prices(pool: &PgPool, mint: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*)::BIGINT FROM token_prices WHERE mint = $1")
        .bind(mint)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn writing_against_a_compressed_chunk_still_behaves(pool: PgPool) {
    let mint = pk(2).to_string();
    let at = Utc::now() - Duration::days(20);

    price_mint_since(&pool, &mint, "1.5", SEEDED_DAYS * 24).await;

    let existing: DateTime<Utc> = sqlx::query_scalar(
        "SELECT fetched_at FROM token_prices
          WHERE mint = $1 AND fetched_at <= $2
          ORDER BY fetched_at DESC LIMIT 1",
    )
    .bind(&mint)
    .bind(at)
    .fetch_one(&pool)
    .await
    .unwrap();

    compress_every_chunk(&pool).await;

    let before = count_prices(&pool, &mint).await;

    // The real batch writer: one row already there is dropped, one new is stored, both
    // inside a compressed chunk.
    let price_at = |at: DateTime<Utc>| TokenPrice {
        mint: pk(2),
        price_usd: Decimal::new(9, 0),
        price_provider: PriceProvider::Jupiter,
        confidence: Some(0.75),
        fetched_at: at,
    };
    PgTokenPriceRepository::new(pool.clone())
        .insert_batch(&[
            price_at(existing),
            price_at(existing + Duration::minutes(7)),
        ])
        .await
        .expect("the batch insert must not fail against a compressed chunk");
    assert_eq!(
        count_prices(&pool, &mint).await,
        before + 1,
        "the conflicting row must be dropped and the new one stored — one row \
         written out of the two offered"
    );

    // Raw SQL without `ON CONFLICT`: what is under test is the constraint, still enforced.
    let err = sqlx::query(
        "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
         VALUES ($1, 9.0, 'jupiter', $2)",
    )
    .bind(&mint)
    .bind(existing)
    .execute(&pool)
    .await
    .expect_err("a duplicate key must still be rejected on a compressed chunk");
    assert_eq!(
        sqlstate(&err),
        UNIQUE_VIOLATION,
        "the duplicate must be refused by the primary key, not by something else"
    );
}
