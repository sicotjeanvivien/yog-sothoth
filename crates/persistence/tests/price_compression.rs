//! Integration tests for migration 011 — the price series is compressed past
//! seven days, and compressing it changes nothing a reader can see.
//!
//! Gated behind `integration-tests`. Three things are asserted, and none of
//! them covers another:
//!
//!   1. the **policy as declared**, read out of the TimescaleDB catalog — it is
//!      what fails if a later migration re-declares the delay, the segmenting
//!      column, or puts back the default index this one drops;
//!   2. the **behaviour it has to preserve**: both shapes of price lookup
//!      return the identical value once the chunk they read is compressed.
//!      Baseline §7 refused compression precisely because of those lookups, so
//!      that refusal is what this test replaces;
//!   3. the **write path against a compressed chunk** — the repository's own
//!      batch, carrying one row already there and one new, then a duplicate
//!      written without its `ON CONFLICT` guard.
//!
//! ⚠️ **`compress_chunk` is called by hand here, and that is not a shortcut.**
//! The local Postgres runs with `timescaledb.max_background_workers = 0`, so
//! the policy this migration declares never fires — a test that only declared
//! it would pass on an uncompressed table and prove nothing. That is why test 2
//! asserts `is_compressed` before re-reading: remove the compression step and
//! it must go red there, not silently keep passing.

use super::helpers::{UNIQUE_VIOLATION, pk, sqlstate};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};

use yog_core::domain::{PriceProvider, TokenPrice, TokenPriceLookup, TokenPriceRepository};
use yog_persistence::PgTokenPriceRepository;

/// Days of hourly price observations seeded by the behaviour tests. The
/// hypertable's chunks span 7 days, so 30 days is several chunks plus the open
/// one — compressing all of them is what puts a compressed chunk under the
/// *latest* lookup, which has no time bound and would otherwise only ever read
/// the hot chunk.
const SEEDED_DAYS: i64 = 30;

// ── 1. The policy, as the catalog holds it ───────────────────────────────────

/// One row of `timescaledb_information.compression_settings`: a column, and the
/// place it takes in the compression layout. A column is either a segment key
/// or part of the order inside a segment, so exactly one of the two indexes is
/// set on each row — which is why both are `Option`.
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
    let compress_after: Vec<String> = sqlx::query_scalar(
        "SELECT config->>'compress_after'
           FROM timescaledb_information.jobs
          WHERE proc_name = 'policy_compression'
            AND hypertable_name = 'token_prices'",
    )
    .fetch_all(&pool)
    .await
    .expect("the compression policy must be readable from the catalog");

    assert_eq!(
        compress_after,
        vec!["7 days".to_string()],
        "token_prices must carry exactly one compression policy, at the house \
         delay of 7 days — the one every other hypertable `001` compresses \
         uses, `signals` excepted at 30 days"
    );

    // The other half of baseline §7 still stands, and its absence is a decision:
    // migration 005 established that an as-of gap never heals (the worker only
    // inserts at now(), nothing backfills), so dropping price rows would blank
    // the valuation of every bucket they cover, for ever.
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

    // `(mint, fetched_at)` is also the primary key, which is what keeps this
    // table free of the `column … should be used for segmenting or ordering`
    // warnings the event tables raise: uniqueness can be checked against
    // compressed rows without decompressing the segment.
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
         and read by nothing — must be gone, and the two written indexes must \
         both still be there"
    );
}

// ── 2. The behaviour compression must not change ─────────────────────────────

/// What the two shapes of price lookup return, side by side.
///
/// NUMERIC values are compared as their textual rendering: the point is that
/// compression returns the *identical* value, and a text comparison says so
/// without going through a decimal type that could round both sides the same
/// way.
#[derive(Debug, PartialEq, Eq)]
struct Readings {
    /// `pool_current_tvl` — the *latest* shape: no time bound at all.
    tvl_usd: Option<String>,
    /// `meteora_damm_v2_pool_hourly_activity` — the *as-of* shape, bounded to
    /// one hour before each bucket.
    buckets: i64,
    volume_usd: Option<String>,
    /// The repository read path the API uses, `TokenPriceLookup`.
    latest_price: Option<String>,
    latest_fetched_at: Option<DateTime<Utc>>,
    latest_confidence: Option<String>,
    /// Every `confidence` in the table, in key order. It is the only nullable
    /// column and the only `REAL` one, so it is the only one compression stores
    /// down a codec path the other columns never take — and a comparison over
    /// the columns that are always populated would never notice.
    confidence_digest: Option<String>,
}

async fn read_both_shapes(pool: &PgPool, pool_addr: &str) -> Readings {
    let tvl_usd: Option<String> =
        sqlx::query_scalar("SELECT tvl_usd::TEXT FROM pool_current_tvl WHERE pool_address = $1")
            .bind(pool_addr)
            .fetch_one(pool)
            .await
            .expect("the pool must be present in pool_current_tvl");

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
        tvl_usd,
        buckets: activity.get("buckets"),
        volume_usd: activity.get("volume_usd"),
        latest_price: latest.as_ref().map(|p| p.price_usd.to_string()),
        latest_fetched_at: latest.as_ref().map(|p| p.fetched_at),
        latest_confidence: latest.and_then(|p| p.confidence).map(|c| c.to_string()),
        confidence_digest,
    }
}

/// A pool with resolved mints, reserves, and `SEEDED_DAYS` of hourly prices and
/// swaps — enough for both lookup shapes to return a number, over several
/// chunks.
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

    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b, last_sqrt_price, last_swap_at, last_slot, last_event_index)
         VALUES ($1,'meteora_damm_v2',NOW(),'swap','sig',
                 1000000000, 2000000000000, 18446744073709551616, NOW(), 1, 0)",
    )
    .bind(&pool_addr)
    .execute(pool)
    .await
    .unwrap();

    // One observation per hour per mint. A price moving from hour to hour is
    // what makes the comparison meaningful: a constant series would read the
    // same whichever row the lookup picked.
    //
    // `confidence` is populated on the first mint and left NULL on the second,
    // on purpose: it is the table's only nullable column, and both states have
    // to survive a round trip through the compressed form.
    for (mint, base, confidence) in [(&mint_a, "1.5", Some(0.75_f32)), (&mint_b, "100.0", None)] {
        sqlx::query(
            "INSERT INTO token_prices (mint, price_usd, price_provider, confidence, fetched_at)
             SELECT $1, $2::NUMERIC + h / 1000.0, 'jupiter', $4,
                    now() - (h || ' hours')::interval
               FROM generate_series(0, $3::INT * 24) h",
        )
        .bind(mint)
        .bind(base)
        .bind(SEEDED_DAYS as i32)
        .bind(confidence)
        .execute(pool)
        .await
        .unwrap();
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

/// Compress every chunk, the open one included, and assert that they really are
/// compressed.
///
/// The hot chunk is compressed on purpose although the policy will never touch
/// it: it is the only way to put a compressed chunk under the *latest* lookup,
/// which carries no time bound and therefore reads the newest chunk first.
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
        before.tvl_usd.is_some()
            && before.volume_usd.is_some()
            && before.buckets > 0
            && before.latest_confidence.is_some(),
        "the fixture must produce a valued TVL, valued buckets and a non-NULL \
         confidence before anything is compressed, or the comparison below \
         compares two absences: {before:?}"
    );

    compress_every_chunk(&pool).await;

    let after = read_both_shapes(&pool, &pool_addr).await;
    assert_eq!(
        after, before,
        "compression must be invisible to both shapes of price lookup — the \
         current-price one (pool_current_tvl, no time bound) and the as-of one \
         (the hourly activity view, bounded to one hour before each bucket). \
         Baseline §7 refused compression for fear of exactly these lookups"
    );
}

// ── 3. Writing against a compressed chunk ────────────────────────────────────

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

    sqlx::query(
        "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
         SELECT $1, 1.5, 'jupiter', now() - (h || ' hours')::interval
           FROM generate_series(0, $2::INT * 24) h",
    )
    .bind(&mint)
    .bind(SEEDED_DAYS as i32)
    .execute(&pool)
    .await
    .unwrap();

    // The row this test then writes against, taken from the seeded series so it
    // sits inside a chunk that is about to be compressed.
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

    // The real writer, not a copy of its SQL: `insert_batch` is a multi-row
    // `QueryBuilder` INSERT carrying `ON CONFLICT (mint, fetched_at) DO
    // NOTHING`, and the batch is the shape that matters — TimescaleDB
    // decompresses per candidate segment to check uniqueness. One row already
    // there, one not: the first must be dropped and the second stored, both
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

    // And without that guard, uniqueness is still ENFORCED there. The warning
    // TimescaleDB raises on the event tables is about cost, not correctness —
    // and this table raises none at all, its key being the segmentby/orderby
    // pair. Written as raw SQL because no repository ever inserts without the
    // conflict clause: what is under test here is the constraint, not a caller.
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
