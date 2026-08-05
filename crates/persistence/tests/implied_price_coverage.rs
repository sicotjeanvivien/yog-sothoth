//! Integration tests for migration 002 — valuing a swap bucket by whichever
//! side is priced, and reporting the coverage of the resulting sums.
//!
//! Gated behind `integration-tests`. Covers the finding of `.project` ticket 02:
//! one missing price used to annihilate the whole bucket (NULL is contagious in
//! SQL arithmetic), and `SUM` then skipped it — publishing a sub-total as a
//! total, silently.
//!
//! Two distinct guarantees are asserted here, and they pull in opposite
//! directions on purpose:
//!
//!   1. a bucket with ONE side priced is now valued, through the exchange rate
//!      its own swaps traded at;
//!   2. a bucket with NEITHER side priced stays NULL — "we don't know" survives,
//!      no fallback onto a later price was introduced.
//!
//! # A note on the fixture amounts
//!
//! Unlike `volume_cagg.rs`, the swaps that carry volume here carry BOTH legs. A
//! swap with a zero counter-leg cannot happen on chain, and it is precisely the
//! counter-leg that anchors the valuation — a fixture that omits it would test
//! nothing. The one exception is the fee test, which adds a leg-less swap on
//! purpose: it contributes only a fee, so the implied rate under test stays the
//! one the other two swaps established.

use super::helpers::pk;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use yog_core::domain::PoolAnalyticsRepository;
use yog_persistence::PgPoolAnalyticsRepository;

/// Token A is a 6-decimal "memecoin", token B a 9-decimal "SOL".
const DEC_A: i16 = 6;
const DEC_B: i16 = 9;
/// The anchor price used throughout: $180 per unit of token B.
const PRICE_B: &str = "180.0";

async fn insert_pool(pool: &PgPool, pool_addr: &str, mint_a: &str, mint_b: &str) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(pool_addr)
    .bind(mint_a)
    .bind(mint_b)
    .execute(pool)
    .await
    .unwrap();

    for (mint, decimals) in [(mint_a, DEC_A), (mint_b, DEC_B)] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1,$2,$3,$3)",
        )
        .bind(mint)
        .bind(decimals)
        .bind(Utc::now() - Duration::hours(48))
        .execute(pool)
        .await
        .unwrap();
    }
}

/// Price a mint well before any bucket, so the as-of lookup hits it.
async fn price_mint(pool: &PgPool, mint: &str, price: &str) {
    sqlx::query(
        "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
         VALUES ($1,$2::NUMERIC,'jupiter',$3)",
    )
    .bind(mint)
    .bind(price)
    .bind(Utc::now() - Duration::hours(48))
    .execute(pool)
    .await
    .unwrap();
}

/// Parameters of one swap insert. A struct rather than eight positional
/// arguments — the house rule, and here it also keeps the fixtures readable.
struct Swap<'a> {
    pool_addr: &'a str,
    signature: &'a str,
    direction: &'a str,
    amount_a: i64,
    amount_b: i64,
    fee_a: i64,
    fee_token_is_a: bool,
    timestamp: DateTime<Utc>,
}

async fn insert_swap(pool: &PgPool, s: Swap<'_>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1,$2,$3,$4,$5,0,0,0,$6,0,0,0,$7,$8,0,0)",
    )
    .bind(s.pool_addr)
    .bind(s.signature)
    .bind(s.direction)
    .bind(s.amount_a)
    .bind(s.amount_b)
    .bind(s.fee_a)
    .bind(s.fee_token_is_a)
    .bind(s.timestamp)
    .execute(pool)
    .await
    .unwrap();
}

/// A pair of swaps totalling 3 000 token A against 1.5 token B, in one bucket.
///
/// The implied rate is therefore `1.5 × $180 / 3000 = $0.09` per token A, and
/// the volume convention (input side only) gives:
///   - `a_to_b` input 1 000 A × $0.09 = $90
///   - `b_to_a` input 1.0 B  × $180   = $180
///
/// → **$270**, which is also exactly the token B that changed hands. The
/// valuation is anchored on the hard asset, as intended.
async fn insert_balanced_pair(pool: &PgPool, pool_addr: &str, tag: &str, at: DateTime<Utc>) {
    insert_swap(
        pool,
        Swap {
            pool_addr,
            signature: &format!("{tag}_ab"),
            direction: "a_to_b",
            amount_a: 1_000_000_000, // 1 000 A
            amount_b: 500_000_000,   // 0.5 B
            fee_a: 0,
            fee_token_is_a: true,
            timestamp: at,
        },
    )
    .await;
    insert_swap(
        pool,
        Swap {
            pool_addr,
            signature: &format!("{tag}_ba"),
            direction: "b_to_a",
            amount_a: 2_000_000_000, // 2 000 A
            amount_b: 1_000_000_000, // 1.0 B
            fee_a: 0,
            fee_token_is_a: false,
            timestamp: at,
        },
    )
    .await;
}

fn close_to(got: Decimal, expected: &str) -> bool {
    (got - Decimal::from_str_exact(expected).unwrap()).abs() < Decimal::new(1, 4)
}

// ── 1. One side priced → the bucket is valued ────────────────────────────────

#[sqlx::test]
async fn single_sided_bucket_is_valued_through_its_own_trade_rate(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    // Only token B is priced. Token A is the unlisted memecoin — the dominant
    // shape of the defect on DAMM v2.
    price_mint(&pool, &mint_b, PRICE_B).await;

    insert_balanced_pair(&pool, &pool_addr, "one", Utc::now() - Duration::hours(2)).await;

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(1)]).await.unwrap();
    let a = analytics.get(&pk(1)).expect("pool must be present");

    let volume = a.volume_24h_usd.expect(
        "a bucket with one side priced must be valued — this is exactly the \
         bucket the baseline returned as NULL",
    );
    assert!(
        close_to(volume, "270"),
        "expected the token B notional ($270), got {volume}"
    );
    assert_eq!(a.swap_buckets_24h, 1);
    assert_eq!(
        a.swap_buckets_priced_24h, 1,
        "the bucket is valued, so it counts as covered"
    );
}

#[sqlx::test]
async fn implied_price_is_flagged_as_implied(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    price_mint(&pool, &mint_b, PRICE_B).await;
    insert_balanced_pair(&pool, &pool_addr, "flag", Utc::now() - Duration::hours(2)).await;

    let row = sqlx::query_as::<_, (Option<Decimal>, bool, bool)>(
        "SELECT eff_price_a, price_a_implied, price_b_implied
         FROM meteora_damm_v2_swap_events_hourly_priced WHERE pool_address = $1",
    )
    .bind(&pool_addr)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(
        close_to(row.0.expect("token A must have an effective price"), "0.09"),
        "1.5 B × $180 / 3000 A = $0.09 per A, got {:?}",
        row.0
    );
    assert!(
        row.1,
        "token A's price was derived, so it is flagged implied"
    );
    assert!(
        !row.2,
        "token B's price was observed, so it must NOT be flagged implied"
    );
}

// ── 2. Neither side priced → still NULL, and the coverage says so ────────────

#[sqlx::test]
async fn bucket_with_no_priced_side_stays_null(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    // Deliberately no price at all: nothing anchors the rate.
    insert_balanced_pair(&pool, &pool_addr, "dark", Utc::now() - Duration::hours(2)).await;

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(1)]).await.unwrap();
    let a = analytics.get(&pk(1)).expect("pool must be present");

    assert_eq!(
        a.volume_24h_usd, None,
        "with neither side priced there is nothing to anchor a rate on — \
         'we don't know' must survive the fix"
    );
    assert_eq!(a.swap_buckets_24h, 1, "the hour did trade");
    assert_eq!(
        a.swap_buckets_priced_24h, 0,
        "…and none of it could be valued — the pair of counters is what \
         distinguishes this from a quiet pool"
    );

    let implied: bool = sqlx::query_scalar(
        "SELECT price_a_implied OR price_b_implied
         FROM meteora_damm_v2_swap_events_hourly_priced WHERE pool_address = $1",
    )
    .bind(&pool_addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !implied,
        "nothing was implied here — the flag must mean 'a rate was used', \
         not 'a price was missing'"
    );
}

// ── 3. Partial coverage is reported, not hidden ──────────────────────────────

#[sqlx::test]
async fn partial_coverage_reports_both_counters(pool: PgPool) {
    // The test the ticket asks for by name: three buckets that traded, one of
    // which cannot be valued. The sum must NOT pass for a complete total.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;

    let now = Utc::now();
    // Token B is priced from 3 hours ago onward — so the bucket 5 hours back
    // has no price to anchor on, while the two recent ones do.
    sqlx::query(
        "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
         VALUES ($1,$2::NUMERIC,'jupiter',$3)",
    )
    .bind(&mint_b)
    .bind(PRICE_B)
    .bind(now - Duration::hours(4))
    .execute(&pool)
    .await
    .unwrap();

    insert_balanced_pair(&pool, &pool_addr, "old", now - Duration::hours(6)).await;
    insert_balanced_pair(&pool, &pool_addr, "mid", now - Duration::hours(3)).await;
    insert_balanced_pair(&pool, &pool_addr, "new", now - Duration::hours(1)).await;

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(1)]).await.unwrap();
    let a = analytics.get(&pk(1)).expect("pool must be present");

    let volume = a.volume_24h_usd.expect("two buckets are valuable");
    assert!(
        close_to(volume, "540"),
        "the two valued buckets total $540, got {volume}"
    );
    assert_eq!(a.swap_buckets_24h, 3, "three hours traded");
    assert_eq!(
        a.swap_buckets_priced_24h, 2,
        "only two could be valued — reporting 3/3 here would be the very \
         defect this pair exists to prevent"
    );
}

// ── 4. The coverage denominator counts swap hours, not any activity ──────────

#[sqlx::test]
async fn coverage_denominator_ignores_liquidity_only_buckets(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    price_mint(&pool, &mint_b, PRICE_B).await;

    let now = Utc::now();
    insert_balanced_pair(&pool, &pool_addr, "swaps", now - Duration::hours(1)).await;

    // A different hour holding only a liquidity event. The activity view unions
    // the buckets of all four caggs, so this bucket exists there — but it is
    // not a *volume* coverage failure and must stay out of the denominator.
    sqlx::query(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind, amount_a, amount_b,
            liquidity_delta, reserve_a_after, reserve_b_after, position, owner,
            timestamp, slot, event_index)
         VALUES ($1,'sig_liq','add',1,1,0,0,0,'pos','own',$2,0,0)",
    )
    .bind(&pool_addr)
    .bind(now - Duration::hours(5))
    .execute(&pool)
    .await
    .unwrap();

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(1)]).await.unwrap();
    let a = analytics.get(&pk(1)).expect("pool must be present");

    assert_eq!(
        a.swap_buckets_24h, 1,
        "the liquidity-only hour must not inflate the denominator — it would \
         report a false 1/2 coverage on a fully covered pool"
    );
    assert_eq!(a.swap_buckets_priced_24h, 1);
}

// ── 5. An observed price still wins over an implied one ──────────────────────

#[sqlx::test]
async fn observed_price_wins_over_the_implied_rate(pool: PgPool) {
    // Both sides priced, and the observed rate deliberately DISAGREES with the
    // rate the swaps traded at ($0.50 observed vs $0.09 implied). The valuation
    // must follow the observed prices — the implied rate is a fallback, never a
    // replacement.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    price_mint(&pool, &mint_a, "0.5").await;
    price_mint(&pool, &mint_b, PRICE_B).await;

    insert_balanced_pair(&pool, &pool_addr, "both", Utc::now() - Duration::hours(2)).await;

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(1)]).await.unwrap();
    let a = analytics.get(&pk(1)).expect("pool must be present");

    // 1 000 A × $0.50 + 1.0 B × $180 = $680 (and NOT $270, the implied result).
    let volume = a.volume_24h_usd.expect("both sides priced");
    assert!(
        close_to(volume, "680"),
        "expected the observed-price valuation ($680), got {volume} — \
         ${} would mean the implied rate overrode an observed price",
        "270"
    );

    let flags: (bool, bool) = sqlx::query_as(
        "SELECT price_a_implied, price_b_implied
         FROM meteora_damm_v2_swap_events_hourly_priced WHERE pool_address = $1",
    )
    .bind(&pool_addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        flags,
        (false, false),
        "nothing needed implying when both prices are observed"
    );
}

// ── 6. Fees ride the same valuation as volume ────────────────────────────────

#[sqlx::test]
async fn fees_are_valued_by_the_same_effective_price(pool: PgPool) {
    // Ticket 02 lists fees among the contaminated figures: they share volume's
    // join, so an hour lost to one was lost to both. They must be recovered
    // together too.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    price_mint(&pool, &mint_b, PRICE_B).await;

    let at = Utc::now() - Duration::hours(2);
    insert_balanced_pair(&pool, &pool_addr, "fee", at).await;
    // A third swap charging a 100-unit fee in token A — the unpriced side.
    insert_swap(
        &pool,
        Swap {
            pool_addr: &pool_addr,
            signature: "fee_charged",
            direction: "a_to_b",
            amount_a: 0,
            amount_b: 0,
            fee_a: 100_000_000, // 100 A
            fee_token_is_a: true,
            timestamp: at,
        },
    )
    .await;

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(1)]).await.unwrap();
    let a = analytics.get(&pk(1)).expect("pool must be present");

    // 100 A × $0.09 = $9, through the same implied rate as the volume.
    let fees = a
        .fees_24h_usd
        .expect("a fee charged in the unpriced token must still be valued");
    assert!(close_to(fees, "9"), "expected $9 of fees, got {fees}");
}

// ── 7. An unresolved pool stays in the denominator ───────────────────────────

#[sqlx::test]
async fn a_pool_with_unresolved_mints_counts_as_uncovered_not_as_absent(pool: PgPool) {
    // Found in review. A pool discovered from the swap stream has NULL mints
    // until yog-context's PoolAccountWorker resolves them, and §15's views
    // INNER-join `token_metadata` — so its buckets used to vanish entirely.
    //
    // Vanishing is fine for a *value*, and wrong for a *coverage denominator*:
    // the buckets that disappear are precisely the ones we failed to value, so
    // counting only the survivors would report "100 % covered" over a window
    // whose volume is silently missing — this ticket's defect, one join up.
    let pool_addr = pk(1).to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',NULL,NULL)",
    )
    .bind(&pool_addr)
    .execute(&pool)
    .await
    .unwrap();

    insert_balanced_pair(
        &pool,
        &pool_addr,
        "unresolved",
        Utc::now() - Duration::hours(2),
    )
    .await;

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(1)]).await.unwrap();
    let a = analytics.get(&pk(1)).expect("pool must be present");

    assert_eq!(
        a.volume_24h_usd, None,
        "no decimals and no mints — nothing can be valued"
    );
    assert_eq!(
        a.swap_buckets_24h, 1,
        "the hour traded and must stay countable: with the INNER join it \
         reported 0 buckets, i.e. the same answer as a pool that never traded"
    );
    assert_eq!(
        a.swap_buckets_priced_24h, 0,
        "…and none of it was valued, so coverage is 0/1 and not 0/0"
    );
}

// ── 8. A swap whose pool row is missing still counts ─────────────────────────

#[sqlx::test]
async fn a_swap_without_its_pool_row_still_counts_as_uncovered(pool: PgPool) {
    // Found in self-review. `discover_pool` runs before the swap insert, but it
    // is skip-and-log: the error is warned and the insert proceeds anyway. So a
    // swap CAN exist with no `pools` row, and an INNER join there would make the
    // bucket vanish from both sides of the coverage ratio — the same silent
    // drop the LEFT join on `token_metadata` exists to prevent.
    //
    // No `pools` insert at all here: that is the point.
    let pool_addr = pk(9).to_string();
    insert_balanced_pair(&pool, &pool_addr, "orphan", Utc::now() - Duration::hours(2)).await;

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(9)]).await.unwrap();
    let a = analytics
        .get(&pk(9))
        .expect("pool must be present in the result");

    assert_eq!(a.volume_24h_usd, None, "nothing identifies the tokens");
    assert_eq!(
        a.swap_buckets_24h, 1,
        "the hour traded: it belongs in the denominator even with no pool row"
    );
    assert_eq!(a.swap_buckets_priced_24h, 0);
}

// ── 9. A zero numerator must not fabricate a priced bucket ───────────────────

#[sqlx::test]
async fn a_bucket_that_moved_no_token_b_is_not_counted_as_covered(pool: PgPool) {
    // Found in review. `NULLIF` was on the divisor only, so with `traded_b = 0`
    // and token B priced, `implied_a` came out a clean 0 → `eff_price_a = 0` →
    // `volume_usd = 0`, non-NULL, hence counted in the coverage NUMERATOR.
    // Coverage read 1/1 over a fabricated zero: this ticket's defect, produced
    // by its own fix.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    price_mint(&pool, &mint_b, PRICE_B).await;

    // One swap that moved token A only — nothing anchors token A's rate.
    insert_swap(
        &pool,
        Swap {
            pool_addr: &pool_addr,
            signature: "no_b",
            direction: "a_to_b",
            amount_a: 1_000_000_000,
            amount_b: 0,
            fee_a: 0,
            fee_token_is_a: true,
            timestamp: Utc::now() - Duration::hours(2),
        },
    )
    .await;

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(1)]).await.unwrap();
    let a = analytics.get(&pk(1)).expect("pool must be present");

    assert_eq!(
        a.volume_24h_usd, None,
        "no token B moved, so no rate can be implied for token A — a zero here \
         is not a valuation, it is an invention"
    );
    assert_eq!(a.swap_buckets_24h, 1);
    assert_eq!(
        a.swap_buckets_priced_24h, 0,
        "and above all it must not count as covered"
    );

    let implied: bool = sqlx::query_scalar(
        "SELECT price_a_implied FROM meteora_damm_v2_swap_events_hourly_priced
         WHERE pool_address = $1",
    )
    .bind(&pool_addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!implied, "nothing was implied — the flag must say so");
}

// ── 10. A zero price is not a price ──────────────────────────────────────────

#[sqlx::test]
async fn a_price_rounded_to_zero_does_not_fabricate_coverage(pool: PgPool) {
    // Found in review. `token_prices.price_usd` is NUMERIC(38,18) with no
    // `CHECK (> 0)`, so any price below 5e-19 is stored as exactly 0 — the
    // regime of very-high-supply memecoins, i.e. the population this migration
    // exists to rescue. `NULLIF` guarded the amounts but not the price, so
    // `implied_a = (traded_b × 0) / traded_a = 0` produced a bucket valued at 0
    // and counted as COVERED.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    // Rounds to exactly 0 in the column's scale.
    price_mint(&pool, &mint_b, "0.00000000000000000000123").await;

    insert_balanced_pair(&pool, &pool_addr, "zero", Utc::now() - Duration::hours(2)).await;

    let stored: Decimal = sqlx::query_scalar("SELECT price_usd FROM token_prices WHERE mint = $1")
        .bind(&mint_b)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, Decimal::ZERO, "precondition: the price stored as 0");

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(1)]).await.unwrap();
    let a = analytics.get(&pk(1)).expect("pool must be present");

    assert_eq!(
        a.volume_24h_usd, None,
        "a zero price anchors nothing — valuing at 0 would be an invention"
    );
    assert_eq!(a.swap_buckets_24h, 1);
    assert_eq!(
        a.swap_buckets_priced_24h, 0,
        "and it must not count as covered"
    );
}

// ── 11. A leg that carries nothing must not annihilate the bucket ────────────

#[sqlx::test]
async fn an_empty_leg_does_not_cancel_a_computable_bucket(pool: PgPool) {
    // Found in review. `0 * NULL` is NULL, and so is `0 / POWER(10, NULL)`, so
    // a leg carrying NO tokens was killing buckets fully computable from the
    // other side. Here token B has no `token_metadata` row (the metadata worker
    // upserts mint by mint and absorbs per-row failures) and the pool traded
    // a→b only: nothing about the B side needs converting, because there is
    // no B.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(&pool_addr)
    .bind(&mint_a)
    .bind(&mint_b)
    .execute(&pool)
    .await
    .unwrap();
    // Only token A gets metadata + a price. Token B has neither.
    sqlx::query(
        "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
         VALUES ($1,$2,$3,$3)",
    )
    .bind(&mint_a)
    .bind(DEC_A)
    .bind(Utc::now() - Duration::hours(48))
    .execute(&pool)
    .await
    .unwrap();
    price_mint(&pool, &mint_a, "2.0").await;

    // a_to_b only: 1 000 A in, and the b_to_a direction never fires.
    insert_swap(
        &pool,
        Swap {
            pool_addr: &pool_addr,
            signature: "one_way",
            direction: "a_to_b",
            amount_a: 1_000_000_000,
            amount_b: 500_000_000,
            fee_a: 0,
            fee_token_is_a: true,
            timestamp: Utc::now() - Duration::hours(2),
        },
    )
    .await;

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let analytics = repo.batch_compute(&[pk(1)]).await.unwrap();
    let a = analytics.get(&pk(1)).expect("pool must be present");

    // 1 000 A × $2 = $2 000. The b_to_a leg is empty, so it contributes 0 —
    // not NULL.
    let volume = a
        .volume_24h_usd
        .expect("the A side is fully priced; an absent B leg must not erase it");
    assert!(
        close_to(volume, "2000"),
        "expected $2000 from the A side alone, got {volume}"
    );
    assert_eq!(a.swap_buckets_priced_24h, 1);
}
