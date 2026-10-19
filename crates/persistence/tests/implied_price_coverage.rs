//! A swap bucket valued by whichever side is priced, through the rate its own swaps
//! traded at. A bucket with neither side priced stays NULL. ⚠️ Every fixture swap is one the chain can produce: both legs carry an
//! amount, the fee side follows `collect_fee_mode`, no fee is zero — an impossible
//! fixture passes for the wrong reason.

use super::helpers::{
    allow_zero_prices, pk, price_mint, price_mint_since, swap_activity, the_only_bucket,
};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

/// Token A is a 6-decimal "memecoin", token B a 9-decimal "SOL".
const DEC_A: i16 = 6;
const DEC_B: i16 = 9;
/// The anchor price used throughout: $180 per unit of token B.
const PRICE_B: &str = "180.0";

/// Hourly prices that store as zero.
///
/// ⚠️ Raw SQL: `insert_batch` refuses a price below the column's scale.
async fn price_mint_as_zero(pool: &PgPool, mint: &str) {
    sqlx::query(
        "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
         SELECT $1, 0.00000000000000000000123, 'jupiter', now() - h * INTERVAL '1 hour'
           FROM generate_series(0, 48) h",
    )
    .bind(mint)
    .execute(pool)
    .await
    .unwrap();
}

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

/// Parameters of one swap insert.
struct Swap<'a> {
    pool_addr: &'a str,
    signature: &'a str,
    direction: &'a str,
    amount_a: i64,
    amount_b: i64,
    /// Fee amount, in whichever token `fee_token_is_a` designates.
    fee: i64,
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
    .bind(s.fee)
    .bind(s.fee_token_is_a)
    .bind(s.timestamp)
    .execute(pool)
    .await
    .unwrap();
}

/// Two swaps, 3 000 A against 1.5 B in one bucket: implied $0.09 per A, volume $270
/// (input side only), fees `100 A × $0.09 + 0.005 B × $180 = $9.90`. ⚠️ Each fee is on the
/// OUT token (`collect_fee_mode = 0`): `a_to_b` with the fee on A exists in no mode.
async fn insert_balanced_pair(pool: &PgPool, pool_addr: &str, tag: &str, at: DateTime<Utc>) {
    insert_swap(
        pool,
        Swap {
            pool_addr,
            signature: &format!("{tag}_ab"),
            direction: "a_to_b",
            amount_a: 1_000_000_000, // 1 000 A
            amount_b: 500_000_000,   // 0.5 B
            fee: 5_000_000,          // 0.005 B — out token
            fee_token_is_a: false,
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
            fee: 100_000_000,        // 100 A — out token
            fee_token_is_a: true,
            timestamp: at,
        },
    )
    .await;
}

fn close_to(got: Decimal, expected: &str) -> bool {
    (got - Decimal::from_str_exact(expected).unwrap()).abs() < Decimal::new(1, 4)
}

#[sqlx::test]
async fn single_sided_bucket_is_valued_through_its_own_trade_rate(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    // Only token B is priced; token A is the unlisted memecoin.
    price_mint(&pool, &mint_b, PRICE_B).await;

    insert_balanced_pair(&pool, &pool_addr, "one", Utc::now() - Duration::hours(2)).await;

    let a = the_only_bucket(&pool, &pool_addr).await;

    let volume = a.volume_usd.expect(
        "a bucket with one side priced must be valued — this is exactly the \
         bucket the baseline returned as NULL",
    );
    assert!(
        close_to(volume, "270"),
        "expected the token B notional ($270), got {volume}"
    );
    assert_eq!(a.swap_count, Some(2));
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

#[sqlx::test]
async fn bucket_with_no_priced_side_stays_null(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    insert_balanced_pair(&pool, &pool_addr, "dark", Utc::now() - Duration::hours(2)).await;

    let a = the_only_bucket(&pool, &pool_addr).await;

    assert_eq!(
        a.volume_usd, None,
        "with neither side priced there is nothing to anchor a rate on — \
         'we don't know' must survive the fix"
    );
    assert_eq!(a.swap_count, Some(2), "the hour did trade");

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

#[sqlx::test]
async fn only_the_priced_buckets_are_valued(pool: PgPool) {
    // Three buckets traded, one cannot be valued.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;

    let now = Utc::now();
    // ⚠️ A continuous series from -4h: a lone observation would leave the -1h bucket
    // unpriced too, and the test would pass for the wrong reason.
    price_mint_since(&pool, &mint_b, PRICE_B, 4).await;

    insert_balanced_pair(&pool, &pool_addr, "old", now - Duration::hours(6)).await;
    insert_balanced_pair(&pool, &pool_addr, "mid", now - Duration::hours(3)).await;
    insert_balanced_pair(&pool, &pool_addr, "new", now - Duration::hours(1)).await;

    let volumes: Vec<_> = swap_activity(&pool, &pool_addr)
        .await
        .into_iter()
        .map(|b| b.volume_usd)
        .collect();
    assert_eq!(volumes.len(), 3, "three hours traded");
    assert_eq!(
        volumes[0], None,
        "the hour before the price series cannot be valued — a value here \
         would be the very defect this test exists to prevent"
    );
    for volume in &volumes[1..] {
        let volume = volume.expect("the two later buckets are valuable");
        assert!(close_to(volume, "270"), "expected $270 each, got {volume}");
    }
}

#[sqlx::test]
async fn a_liquidity_only_bucket_carries_no_swap_count(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    price_mint(&pool, &mint_b, PRICE_B).await;

    let now = Utc::now();
    insert_balanced_pair(&pool, &pool_addr, "swaps", now - Duration::hours(1)).await;

    // An hour holding only a liquidity event: it exists in the activity view.
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

    let buckets = swap_activity(&pool, &pool_addr).await;
    assert_eq!(buckets.len(), 2, "{buckets:?}");
    assert_eq!(
        buckets[0].swap_count, None,
        "the liquidity-only hour carries no swap count — a reader would count \
         it as an unvalued trading hour"
    );
    assert_eq!(buckets[1].swap_count, Some(2));
    assert!(buckets[1].volume_usd.is_some());
}

#[sqlx::test]
async fn observed_price_wins_over_the_implied_rate(pool: PgPool) {
    // Observed $0.50 against implied $0.09: the implied rate is a fallback, never a
    // replacement.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    price_mint(&pool, &mint_a, "0.5").await;
    price_mint(&pool, &mint_b, PRICE_B).await;

    insert_balanced_pair(&pool, &pool_addr, "both", Utc::now() - Duration::hours(2)).await;

    let a = the_only_bucket(&pool, &pool_addr).await;

    // 1 000 A × $0.50 + 1.0 B × $180 = $680 (and NOT $270, the implied result).
    let volume = a.volume_usd.expect("both sides priced");
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

#[sqlx::test]
async fn fees_are_valued_by_the_same_effective_price(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    price_mint(&pool, &mint_b, PRICE_B).await;

    let at = Utc::now() - Duration::hours(2);
    // Part of the pair's fee lands on the unpriced token A: only the implied rate values it.
    insert_balanced_pair(&pool, &pool_addr, "fee", at).await;

    let a = the_only_bucket(&pool, &pool_addr).await;

    // 100 A × $0.09 (implied) + 0.005 B × $180 (observed) = $9.90.
    let fees = a
        .fees_usd
        .expect("a fee charged in the unpriced token must still be valued");
    assert!(close_to(fees, "9.9"), "expected $9.90 of fees, got {fees}");
}

#[sqlx::test]
async fn a_pool_with_unresolved_mints_counts_as_uncovered_not_as_absent(pool: PgPool) {
    // ⚠️ An unresolved pool's buckets must stay: they are the ones that failed to value,
    // and dropping them would read as 100 % covered.
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

    let a = the_only_bucket(&pool, &pool_addr).await;

    assert_eq!(
        a.volume_usd, None,
        "no decimals and no mints — nothing can be valued"
    );
    assert_eq!(
        a.swap_count,
        Some(2),
        "the hour traded and must stay countable: with the INNER join the \
         bucket was absent, i.e. the same answer as a pool that never traded"
    );
}

#[sqlx::test]
async fn a_swap_without_its_pool_row_still_counts_as_uncovered(pool: PgPool) {
    // No `pools` row on purpose: discovery is skip-and-log, so a swap can exist without
    // one, and an INNER join would drop its bucket.
    let pool_addr = pk(9).to_string();
    insert_balanced_pair(&pool, &pool_addr, "orphan", Utc::now() - Duration::hours(2)).await;

    let a = the_only_bucket(&pool, &pool_addr).await;

    assert_eq!(a.volume_usd, None, "nothing identifies the tokens");
    assert_eq!(
        a.swap_count,
        Some(2),
        "the hour traded: it belongs in the view even with no pool row"
    );
}

#[sqlx::test]
async fn a_bucket_that_moved_no_token_b_is_not_valued(pool: PgPool) {
    // Mutation: `NULLIF` on the divisor only — `traded_b = 0` gives an implied price of 0,
    // a non-NULL `volume_usd`.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    price_mint(&pool, &mint_b, PRICE_B).await;

    insert_swap(
        &pool,
        Swap {
            pool_addr: &pool_addr,
            signature: "no_b",
            direction: "a_to_b",
            amount_a: 1_000_000_000,
            amount_b: 0,
            fee: 2_500_000,
            fee_token_is_a: false, // a_to_b pays on the OUT token, always B
            timestamp: Utc::now() - Duration::hours(2),
        },
    )
    .await;

    let a = the_only_bucket(&pool, &pool_addr).await;

    assert_eq!(
        a.volume_usd, None,
        "no token B moved, so no rate can be implied for token A — a zero here \
         is not a valuation, it is an invention"
    );
    assert_eq!(a.swap_count, Some(1));

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

#[sqlx::test]
async fn a_price_rounded_to_zero_does_not_value_the_bucket(pool: PgPool) {
    // Mutation: no `NULLIF` on the price — a stored zero values the bucket at 0. The column refuses a zero: `helpers::allow_zero_prices` lifts it.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    allow_zero_prices(&pool).await;
    price_mint_as_zero(&pool, &mint_b).await;

    insert_balanced_pair(&pool, &pool_addr, "zero", Utc::now() - Duration::hours(2)).await;

    let stored: Decimal = sqlx::query_scalar("SELECT price_usd FROM token_prices WHERE mint = $1")
        .bind(&mint_b)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, Decimal::ZERO, "precondition: the price stored as 0");

    let a = the_only_bucket(&pool, &pool_addr).await;

    assert_eq!(
        a.volume_usd, None,
        "a zero price anchors nothing — valuing at 0 would be an invention"
    );
    assert_eq!(a.swap_count, Some(2));
}

#[sqlx::test]
async fn an_empty_leg_does_not_cancel_a_computable_bucket(pool: PgPool) {
    // Mutation: a leg carrying no token converted anyway — token A has no metadata,
    // `0 * NULL` is NULL, and the bucket computable from B alone would vanish.
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
    sqlx::query(
        "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
         VALUES ($1,$2,$3,$3)",
    )
    .bind(&mint_b)
    .bind(DEC_B)
    .bind(Utc::now() - Duration::hours(48))
    .execute(&pool)
    .await
    .unwrap();
    price_mint(&pool, &mint_b, PRICE_B).await;

    // b→a with the fee on B (`OnlyB`): the one shape where B alone carries every figure.
    insert_swap(
        &pool,
        Swap {
            pool_addr: &pool_addr,
            signature: "one_way",
            direction: "b_to_a",
            amount_a: 500_000_000,   // 500 A out — A never enters a figure
            amount_b: 1_000_000_000, // 1.0 B in
            fee: 2_500_000,          // 0.0025 B
            fee_token_is_a: false,
            timestamp: Utc::now() - Duration::hours(2),
        },
    )
    .await;

    let a = the_only_bucket(&pool, &pool_addr).await;

    // 1.0 B × $180 = $180.
    let volume = a
        .volume_usd
        .expect("the B side carries everything; an unused A side must not erase it");
    assert!(
        close_to(volume, "180"),
        "expected $180 from the B side alone, got {volume}"
    );
    assert!(a.fees_usd.is_some());
    assert!(a.protocol_fees_usd.is_some());
}

#[sqlx::test]
async fn volume_and_every_fee_share_are_null_together(pool: PgPool) {
    // Mutation: the zero-leg CASE applied per figure, not per bucket — the figures draw on
    // different amounts and would diverge (a NULL volume next to a fee).
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
    sqlx::query(
        "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
         VALUES ($1,$2,$3,$3)",
    )
    .bind(&mint_b)
    .bind(DEC_B)
    .bind(Utc::now() - Duration::hours(48))
    .execute(&pool)
    .await
    .unwrap();
    price_mint(&pool, &mint_b, PRICE_B).await;

    insert_swap(
        &pool,
        Swap {
            pool_addr: &pool_addr,
            signature: "split",
            direction: "a_to_b",
            amount_a: 1_000_000_000,
            amount_b: 500_000_000,
            fee: 1_000_000,
            fee_token_is_a: false, // the fee lands on the PRICED side
            timestamp: Utc::now() - Duration::hours(2),
        },
    )
    .await;

    let a = the_only_bucket(&pool, &pool_addr).await;

    assert_eq!(
        (
            a.volume_usd,
            a.fees_usd,
            a.protocol_fees_usd,
            a.referral_fees_usd,
            a.lp_fees_usd,
        ),
        (None, None, None, None, None),
        "the volume sits on a token that cannot be valued, so the whole bucket \
         is unvaluable — a fee figure surviving alone would make the split lie"
    );
    assert_eq!(a.swap_count, Some(1));
}

#[sqlx::test]
async fn a_zero_price_yields_to_the_implied_rate_instead_of_winning(pool: PgPool) {
    // Mutation: no `NULLIF` in the COALESCE choosing observed over implied — a zero
    // observed price wins, $180 instead of $270.
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    allow_zero_prices(&pool).await;
    price_mint_as_zero(&pool, &mint_a).await;
    price_mint(&pool, &mint_b, PRICE_B).await;

    insert_balanced_pair(
        &pool,
        &pool_addr,
        "zerowin",
        Utc::now() - Duration::hours(2),
    )
    .await;

    let row = sqlx::query_as::<_, (Option<Decimal>, bool)>(
        "SELECT eff_price_a, price_a_implied
         FROM meteora_damm_v2_swap_events_hourly_priced WHERE pool_address = $1",
    )
    .bind(&pool_addr)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(
        close_to(
            row.0.expect("token A must fall back to the implied rate"),
            "0.09"
        ),
        "expected the implied $0.09, got {:?} — a zero price is not a price",
        row.0
    );
    assert!(row.1, "the fallback was used, so the flag must say so");

    let volume = the_only_bucket(&pool, &pool_addr)
        .await
        .volume_usd
        .expect("both sides are valuable once the zero yields");
    assert!(
        close_to(volume, "270"),
        "expected the full $270; $180 would mean the A leg was valued at zero"
    );
}

/// Against the pricing function no longer inlined (`STRICT`, a `SET`, plpgsql…): Postgres
/// would then call it once per hour, outside the plan of the view and of the facts.
#[sqlx::test]
async fn the_hour_pricing_is_inlined_into_the_view(pool: PgPool) {
    let plan: Vec<String> =
        sqlx::query_scalar("EXPLAIN SELECT * FROM meteora_damm_v2_swap_events_hourly_priced")
            .fetch_all(&pool)
            .await
            .unwrap();
    let plan = plan.join("\n");

    assert!(
        plan.contains("fetched_at"),
        "no price lookup in the plan: {plan}"
    );
    assert!(!plan.contains("Function Scan"), "{plan}");
}
