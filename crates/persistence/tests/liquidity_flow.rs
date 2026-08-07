//! Integration test for the liquidity flow view (baseline §15) and the
//! `PgLiquidityFlowRepository` read path.
//!
//! Gated behind `integration-tests`. Validates the vertical slice: liquidity
//! events land in the raw hypertable, the hourly CA (011) exposes the
//! per-direction sums, `meteora_damm_v2_pool_hourly_liquidity_flow` values
//! each direction (both token legs) at the per-bucket trade-time price, and
//! the repository sums the window and joins the pool's current TVL
//! (`pool_current_tvl`, nullable).

use super::helpers::pk;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use yog_core::domain::LiquidityFlowRepository;
use yog_persistence::PgLiquidityFlowRepository;

async fn insert_liquidity_event(
    pool: &PgPool,
    pool_addr: &str,
    signature: &str,
    kind: &str,
    amount_a: i64,
    amount_b: i64,
    timestamp: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind, amount_a, amount_b,
            liquidity_delta, reserve_a_after, reserve_b_after, position, owner, timestamp, slot, event_index)
         VALUES ($1,$2,$3,$4,$5,0,0,0,'','',$6,0,0)",
    )
    .bind(pool_addr)
    .bind(signature)
    .bind(kind)
    .bind(amount_a)
    .bind(amount_b)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

/// Seed one pool (token A: 6 decimals @ $2, token B: 9 decimals @ $100),
/// priced hourly from `priced_since` up to now, and return its address string.
///
/// The series has to be continuous AND to reach the present, because this test
/// crosses both bounds migration 005 introduced: the as-of one behind
/// `meteora_damm_v2_pool_hourly_liquidity_flow` (no older than one hour before
/// the bucket's start) and the latest one behind `pool_current_tvl` (no older
/// than 15 minutes from now). A lone observation nine hours back satisfied
/// neither — the flow read $0 and the TVL came out NULL.
async fn seed_pool(pool: &PgPool, priced_since: DateTime<Utc>) -> String {
    let pool_addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();

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
        .bind(priced_since)
        .execute(pool)
        .await
        .unwrap();
    }

    let now = Utc::now();
    let hours = (now - priced_since).num_hours().max(0);
    for (mint, price) in [(&mint_a, "2.0"), (&mint_b, "100.0")] {
        for h in 0..=hours {
            sqlx::query(
                "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
                 VALUES ($1,$2::NUMERIC,'jupiter',$3)",
            )
            .bind(mint)
            .bind(price)
            .bind(now - Duration::hours(h))
            .execute(pool)
            .await
            .unwrap();
        }
    }

    pool_addr
}

fn close(got: Decimal, want: i64) -> bool {
    (got - Decimal::from(want)).abs() < Decimal::new(1, 4)
}

#[sqlx::test]
async fn flows_split_directions_window_and_join_tvl(pool: PgPool) {
    let now = Utc::now();
    let price_at = now - Duration::hours(9);
    let pool_addr = seed_pool(&pool, price_at).await;

    // Current state: reserves 10.0 A (6 dec) and 1.0 B (9 dec)
    //   → TVL = 10 × $2 + 1 × $100 = $120.
    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b, last_slot, last_event_index)
         VALUES ($1,'meteora_damm_v2',$2,'liquidity_add','sig',10000000,1000000000,1,0)",
    )
    .bind(&pool_addr)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    // In-window adds: 2.0 A ($4) + 0.1 B ($10) → added_usd = $14.
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_add1",
        "add",
        2_000_000,
        100_000_000,
        now - Duration::hours(2),
    )
    .await;
    // In-window removes over two events:
    //   3.0 A ($6) + 0.5 B ($50), then 1.0 A ($2) → removed_usd = $58.
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_rem1",
        "remove",
        3_000_000,
        500_000_000,
        now - Duration::hours(1),
    )
    .await;
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_rem2",
        "remove",
        1_000_000,
        0,
        now - Duration::hours(3),
    )
    .await;
    // Outside the window — must be excluded.
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_old",
        "remove",
        999_000_000,
        0,
        now - Duration::hours(30),
    )
    .await;

    let repo = PgLiquidityFlowRepository::new(pool.clone());
    let flows = repo
        .liquidity_flow_since(now - Duration::hours(6))
        .await
        .unwrap();

    let flow = flows
        .iter()
        .find(|f| f.pool_address == pk(1))
        .expect("pool with priced liquidity events must be present");

    let added = flow
        .added_usd
        .expect("every bucket in the window is valuable, so the sum is a total");
    let removed = flow
        .removed_usd
        .expect("every bucket in the window is valuable, so the sum is a total");
    assert!(close(added, 14), "added expected ~$14, got {added}");
    assert!(close(removed, 58), "removed expected ~$58, got {removed}");
    let tvl = flow.tvl_usd.expect("TVL must be valued");
    assert!(close(tvl, 120), "TVL expected ~$120, got {tvl}");
}

#[sqlx::test]
async fn pool_without_current_state_has_null_tvl(pool: PgPool) {
    let now = Utc::now();
    let pool_addr = seed_pool(&pool, now - Duration::hours(9)).await;

    // Liquidity movement but NO pool_current_state row (claim-only pool):
    // the flow must surface with tvl_usd = None, not vanish.
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_rem",
        "remove",
        1_000_000,
        0,
        now - Duration::hours(1),
    )
    .await;

    let repo = PgLiquidityFlowRepository::new(pool.clone());
    let flows = repo
        .liquidity_flow_since(now - Duration::hours(6))
        .await
        .unwrap();

    let flow = flows
        .iter()
        .find(|f| f.pool_address == pk(1))
        .expect("flow must be present even without a TVL");
    let removed = flow.removed_usd.expect("the window is fully valuable");
    assert!(close(removed, 2), "removed expected ~$2, got {removed}");
    assert_eq!(flow.tvl_usd, None);
}

#[sqlx::test]
async fn a_partly_unvaluable_window_yields_no_total(pool: PgPool) {
    // The case the 6 August update of `.project` ticket 08 added, and the one
    // that had no coverage at all. It is the SILENT failure, not the loud one:
    //
    //   * the pool's current TVL is perfectly known, so `tvl_drain`'s existing
    //     `tvl_usd` guard does not fire;
    //   * `SUM` skips the unvaluable buckets and the old `COALESCE` dressed the
    //     remainder as a total, so the drain came out UNDER-estimated;
    //   * an under-estimated drain crosses no threshold — the signal is simply
    //     missed, and nothing says a thing.
    //
    // Prices cover the last 2 hours: the removal at -1h is valuable, the one at
    // -5h is not, and the window spans both.
    let now = Utc::now();
    let pool_addr = seed_pool(&pool, now - Duration::hours(2)).await;

    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b, last_slot, last_event_index)
         VALUES ($1,'meteora_damm_v2',$2,'liquidity_remove','sig',10000000,1000000000,1,0)",
    )
    .bind(&pool_addr)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    for (sig, hours_ago) in [("sig_recent", 1), ("sig_dark", 5)] {
        insert_liquidity_event(
            &pool,
            &pool_addr,
            sig,
            "remove",
            1_000_000,
            0,
            now - Duration::hours(hours_ago),
        )
        .await;
    }

    let repo = PgLiquidityFlowRepository::new(pool.clone());
    let flows = repo
        .liquidity_flow_since(now - Duration::hours(24))
        .await
        .unwrap();
    let flow = flows.iter().find(|f| f.pool_address == pk(1)).unwrap();

    assert_eq!(
        flow.removed_usd, None,
        "a window that is only partly valuable measures no drain at all — \
         publishing the valuable half under-states it and loses the signal"
    );
    assert_eq!(flow.added_usd, None, "both directions go absent together");
    assert!(
        flow.tvl_usd.is_some(),
        "the current TVL is known and stays known — it is what makes this the \
         MIXED case, the one the tvl_usd guard cannot catch"
    );
}

#[sqlx::test]
async fn an_untouched_leg_does_not_void_a_valuable_bucket(pool: PgPool) {
    // The asymmetric case: one mint priced, the other not, and an hour that
    // moved only the priced one. Nothing about token B needs converting —
    // there was no B — so the bucket is worth exactly what the A side is worth.
    //
    // Without the empty-leg `CASE`, `(0 / 10^dec_b) * NULL` is NULL and the
    // whole `added_usd` goes NULL **while `valuation_complete` stays TRUE**
    // (B carries no amount, so it is not required). `bool_and` therefore stays
    // true, `SUM` skips the row, and the repository publishes a sub-total —
    // the exact defect this migration claims to make unrepresentable.
    //
    // The previous test cannot catch this: it prices and un-prices both mints
    // together, so the flag and the value always agree there.
    let now = Utc::now();
    let pool_addr = seed_pool(&pool, now - Duration::hours(2)).await;

    // Token B loses its price entirely — a mint Jupiter does not cover.
    sqlx::query("DELETE FROM token_prices WHERE mint = $1")
        .bind(pk(3).to_string())
        .execute(&pool)
        .await
        .unwrap();

    // 2.0 A added (6 dec) at $2, and not one lamport of B.
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_single_sided",
        "add",
        2_000_000,
        0,
        now - Duration::hours(1),
    )
    .await;

    let repo = PgLiquidityFlowRepository::new(pool.clone());
    let flows = repo
        .liquidity_flow_since(now - Duration::hours(24))
        .await
        .unwrap();
    let flow = flows.iter().find(|f| f.pool_address == pk(1)).unwrap();

    let added = flow
        .added_usd
        .expect("the A side is priced and is all that moved — this is knowable");
    assert!(close(added, 4), "2.0 A at $2 is $4, got {added}");
    assert_eq!(
        flow.removed_usd,
        Some(Decimal::ZERO),
        "nothing was removed: zero in any currency, and B's missing price says \
         nothing about it"
    );
}
