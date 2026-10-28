//! The claims of `PgAnalyticRepository` (042): each claim of a position's fees or rewards valued
//! once at its pool's price in the tranche, a reward outside the pool at its market price; and
//! their totals by protocol, read by `PgClaimTotalsRepository`.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::domain::{ClaimTotals, ClaimTotalsLookup, FrozenPeriod, Protocol};
use yog_persistence::PgClaimTotalsRepository;

use super::analytic_liquidity::{dec, settled_hour, setup_inflated_token, setup_pool, value};
use super::helpers::{pk, price_at, setup_roles_and_connect_as};

async fn insert_fee_claim(
    pool: &PgPool,
    address: &str,
    tag: &str,
    (a, b): (i64, i64),
    at: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_claim_position_fee_events
           (pool_address, signature, position, owner, fee_a_claimed, fee_b_claimed,
            timestamp, slot, event_index)
         VALUES ($1, $2, 'position', 'owner', $3, $4, $5, 0, 0)",
    )
    .bind(address)
    .bind(tag)
    .bind(a)
    .bind(b)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_reward_claim(
    pool: &PgPool,
    address: &str,
    tag: &str,
    mint: &str,
    amount: i64,
    at: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_claim_reward_events
           (pool_address, signature, position, owner, mint_reward, reward_index, total_reward,
            timestamp, slot, event_index)
         VALUES ($1, $2, 'position', 'owner', $3, 0, $4, $5, 0, 0)",
    )
    .bind(address)
    .bind(tag)
    .bind(mint)
    .bind(amount)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn fee_claim_usd(pool: &PgPool, tag: &str) -> Option<BigDecimal> {
    sqlx::query_scalar("SELECT amount_usd FROM analytic_position_fee_claims WHERE signature = $1")
        .bind(tag)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn reward_claim_usd(pool: &PgPool, tag: &str) -> Option<BigDecimal> {
    sqlx::query_scalar("SELECT amount_usd FROM analytic_reward_claims WHERE signature = $1")
        .bind(tag)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A token known to the metadata with 6 decimals, outside every pool.
async fn known_token(pool: &PgPool, mint: &str) {
    sqlx::query(
        "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
         VALUES ($1, 6, now(), now())",
    )
    .bind(mint)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the claim valued at market — 10 X at $1 would make it $20; or the ledger left
/// without its claims — their coverage never starting.
#[sqlx::test]
async fn a_fee_claim_takes_its_pools_bounded_price_in_the_tranche(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let pool_x = setup_inflated_token(&pool, tranche).await;
    insert_fee_claim(&pool, &pool_x, "fees", (10_000_000, 100_000_000), tranche).await;

    value(&pool, tranche, tranche).await;

    assert_eq!(fee_claim_usd(&pool, "fees").await, Some(dec("10.1")));
    let measured: bool =
        sqlx::query_scalar("SELECT claims FROM analytic_tranches WHERE bucket = $1")
            .bind(tranche)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(measured, "the tranche must say its claims were valued");
}

/// Mutation: an empty claim left to the price — in a pool never priced it would read as not
/// valued, when it took nothing; or a claim with amounts valued there anyway.
#[sqlx::test]
async fn an_empty_claim_is_worth_zero_and_one_in_a_pool_never_priced_is_not_valued(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let unpriced = setup_pool(&pool, 3, &pk(20).to_string(), &pk(21).to_string()).await;
    insert_fee_claim(&pool, &unpriced, "empty", (0, 0), tranche).await;
    insert_fee_claim(&pool, &unpriced, "unpriced", (1_000_000, 0), tranche).await;

    value(&pool, tranche, tranche).await;

    assert_eq!(fee_claim_usd(&pool, "empty").await, Some(dec("0")));
    assert_eq!(fee_claim_usd(&pool, "unpriced").await, None);
}

/// Mutation: a reward of the pool's own token valued at its market — 10 X at $1, $10 instead of
/// $0.10; or one outside the pool valued at nothing, or at the pool's price of another token.
#[sqlx::test]
async fn a_reward_takes_its_pools_price_and_one_outside_the_pool_its_market(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let pool_x = setup_inflated_token(&pool, tranche).await;
    let (outside, unpriced) = (pk(30).to_string(), pk(31).to_string());
    known_token(&pool, &outside).await;
    known_token(&pool, &unpriced).await;
    price_at(&pool, &outside, "2", tranche - Duration::minutes(1)).await;
    insert_reward_claim(
        &pool,
        &pool_x,
        "in-pool",
        &pk(10).to_string(),
        10_000_000,
        tranche,
    )
    .await;
    insert_reward_claim(&pool, &pool_x, "outside", &outside, 3_000_000, tranche).await;
    insert_reward_claim(&pool, &pool_x, "no-price", &unpriced, 3_000_000, tranche).await;

    value(&pool, tranche, tranche).await;

    assert_eq!(reward_claim_usd(&pool, "in-pool").await, Some(dec("0.1")));
    assert_eq!(reward_claim_usd(&pool, "outside").await, Some(dec("6")));
    assert_eq!(reward_claim_usd(&pool, "no-price").await, None);
}

fn at(hours: i64) -> DateTime<Utc> {
    settled_hour() - Duration::hours(hours)
}

async fn insert_valued(
    pool: &PgPool,
    rewards: bool,
    (tag, protocol, owner): (&str, &str, &str),
    amount: Option<&str>,
    at: DateTime<Utc>,
) {
    let sql = if rewards {
        "INSERT INTO analytic_reward_claims (signature, event_index, timestamp, bucket,
           pool_address, protocol, owner, amount_usd, mint_reward)
         VALUES ($1, 0, $2, $2, 'pool', $3, $4, $5::NUMERIC, 'mint')"
    } else {
        "INSERT INTO analytic_position_fee_claims (signature, event_index, timestamp, bucket,
           pool_address, protocol, owner, amount_usd)
         VALUES ($1, 0, $2, $2, 'pool', $3, $4, $5::NUMERIC)"
    };
    sqlx::query(sql)
        .bind(tag)
        .bind(at)
        .bind(protocol)
        .bind(owner)
        .bind(amount)
        .execute(pool)
        .await
        .unwrap();
}

/// Mutation: either bound of the period dropped, the protocol not filtered, the owners counted
/// per claim, or an unvalued claim counted as valued — each read off the totals below.
#[sqlx::test]
async fn the_totals_sum_the_claims_of_their_protocol_over_the_period(pool: PgPool) {
    for rewards in [false, true] {
        let damm = "meteora_damm_v2";
        insert_valued(&pool, rewards, ("a", damm, "alice"), Some("10"), at(3)).await;
        insert_valued(&pool, rewards, ("b", damm, "alice"), Some("5"), at(2)).await;
        insert_valued(&pool, rewards, ("c", damm, "bob"), None, at(2)).await;
        insert_valued(
            &pool,
            rewards,
            ("before", damm, "carol"),
            Some("100"),
            at(30),
        )
        .await;
        insert_valued(&pool, rewards, ("at-end", damm, "dave"), Some("100"), at(0)).await;
        insert_valued(
            &pool,
            rewards,
            ("dlmm", "meteora_dlmm", "erin"),
            Some("100"),
            at(2),
        )
        .await;
    }
    let repository =
        PgClaimTotalsRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let period = FrozenPeriod {
        from: at(24),
        as_of: at(0),
        covered_since: at(24),
    };
    let expected = ClaimTotals {
        claims: 3,
        claims_valued: 2,
        amount_usd: Decimal::from(15),
        owners: 2,
    };

    let fees = repository
        .find_position_fee_claim_totals(&period, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let rewards = repository
        .find_reward_claim_totals(&period, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(fees, expected);
    assert_eq!(rewards, expected);
}

/// Mutation: the coverage read from the claims themselves, or from every tranche — a tranche
/// without a claim not observed, or one before the claims were valued counted as measured.
#[sqlx::test]
async fn the_claims_are_measured_from_the_first_tranche_whose_claims_were_valued(pool: PgPool) {
    for (bucket, claims) in [(at(5), false), (at(4), true), (at(3), true)] {
        sqlx::query(
            "INSERT INTO analytic_tranches (bucket, written_at, claims) VALUES ($1, $1, $2)",
        )
        .bind(bucket)
        .bind(claims)
        .execute(&pool)
        .await
        .unwrap();
    }
    let repository =
        PgClaimTotalsRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let damm = repository
        .find_measured_since(Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let dlmm = repository
        .find_measured_since(Protocol::MeteoraDlmm)
        .await
        .unwrap();

    assert_eq!(damm, Some(at(4)));
    assert_eq!(dlmm, None);
}
