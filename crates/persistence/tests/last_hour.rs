//! The last hour under `yog_api`: the twelve tranches through the last one written, read from
//! each measure's facts — the pools' tranches, the moves, the claims, the creations and the
//! position events themselves — and the new series of the frozen hours.

use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::Cursor;
use yog_core::domain::{
    ClaimKind, ClaimTotalsLookup, FrozenPeriod, LiquidityMoveList, LiquidityMoveLookup,
    LiquidityMoveOrder, LiquidityMoveScope, PoolCreationsLookup, PositionChangeCountsLookup,
    Protocol, ProtocolActivityLookup, RotationRunLookup, SwapActivityLookup, TokenActivityFeed,
    TranchePeriod, Window,
};
use yog_persistence::{
    PgClaimTotalsRepository, PgLiquidityMoveRepository, PgPoolCreationsRepository,
    PgPositionChangeCountsRepository, PgProtocolActivityRepository, PgRotationRunRepository,
    PgSwapActivityRepository, PgTokenActivityRepository,
};

use super::helpers::{pk, setup_roles_and_connect_as};

/// The end of the last tranche written.
fn through() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 15, 0).unwrap()
}

fn minutes_before(minutes: i64) -> DateTime<Utc> {
    through() - TimeDelta::minutes(minutes)
}

/// Every tranche of the three hours before `through()` written.
async fn write_tranches(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO analytic_tranches (bucket, written_at)
         SELECT generate_series($1::TIMESTAMPTZ - INTERVAL '3 hours',
                                $1::TIMESTAMPTZ - INTERVAL '5 minutes', INTERVAL '5 minutes'), now()",
    )
    .bind(through())
    .execute(pool)
    .await
    .unwrap();
}

/// A pool of DAMM v2 trading `a` against `b`.
async fn seed_pool(pool: &PgPool, address: &str, a: &str, b: &str) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(address)
    .bind(a)
    .bind(b)
    .execute(pool)
    .await
    .unwrap();
}

/// One pool's tranche at `bucket`: `swaps` swaps worth `usd`, valued unless `usd` is `None`.
async fn tranche(
    pool: &PgPool,
    address: &str,
    bucket: DateTime<Utc>,
    swaps: i64,
    usd: Option<i64>,
) {
    sqlx::query(
        "INSERT INTO analytic_pool_5min
           (pool_address, bucket, protocol, swap_count, valued, volume_in_a_usd, volume_in_b_usd,
            fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd)
         VALUES ($1, $2, 'meteora_damm_v2', $3, $4, $5, 0, $5, $5, 0, 0)",
    )
    .bind(address)
    .bind(bucket)
    .bind(swaps)
    .bind(usd.is_some())
    .bind(usd.map(BigDecimal::from))
    .execute(pool)
    .await
    .unwrap();
}

async fn last_hour(pool: &PgPool) -> TranchePeriod {
    let repo = PgProtocolActivityRepository::new(setup_roles_and_connect_as(pool, "yog_api").await);
    TranchePeriod::hour_through(repo.written_tranches().await.unwrap().unwrap(), None)
}

/// Against the hour cut at 11 or 13 tranches, or pools counted per tranche: the tranche just
/// before the hour and the one at its end stay out, a pool active in two tranches counts once,
/// and an unvalued tranche adds nothing to the volume.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hour_sums_its_twelve_tranches_and_counts_its_pools_once(pool: PgPool) {
    write_tranches(&pool).await;
    let (x, y) = (pk(1).to_string(), pk(2).to_string());
    seed_pool(&pool, &x, &pk(10).to_string(), &pk(11).to_string()).await;
    seed_pool(&pool, &y, &pk(10).to_string(), &pk(12).to_string()).await;
    tranche(&pool, &x, minutes_before(65), 1000, Some(1000)).await;
    tranche(&pool, &x, minutes_before(60), 1, Some(10)).await;
    tranche(&pool, &x, minutes_before(5), 2, Some(20)).await;
    tranche(&pool, &y, minutes_before(30), 4, None).await;
    tranche(&pool, &x, through(), 1000, Some(1000)).await;

    let period = last_hour(&pool).await;
    let repo = PgSwapActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let totals = repo
        .find_protocol_totals_in_tranches(&period, None)
        .await
        .unwrap();

    assert_eq!(period.as_of, through());
    assert_eq!(totals.len(), 1);
    let activity = &totals[0].activity;
    assert_eq!(activity.swaps, 7);
    assert_eq!(activity.volume_usd, Some(Decimal::from(30)));
    assert_eq!((activity.pool_hours, activity.pool_hours_valued), (3, 2));
    assert_eq!(totals[0].pools, Some(2));
}

/// Against a tranche without a pool left out of the series: the hour has its twelve bars, 0 where
/// nothing traded; and against the hour before read where it is not.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hour_has_twelve_bars_and_the_hour_before_its_own(pool: PgPool) {
    write_tranches(&pool).await;
    let x = pk(1).to_string();
    seed_pool(&pool, &x, &pk(10).to_string(), &pk(11).to_string()).await;
    tranche(&pool, &x, minutes_before(5), 2, Some(20)).await;
    tranche(&pool, &x, minutes_before(65), 3, Some(30)).await;

    let period = last_hour(&pool).await;
    let tranches = PgProtocolActivityRepository::new(pool.clone())
        .written_tranches()
        .await
        .unwrap()
        .unwrap();
    let before = TranchePeriod::hour_through(tranches, Some(Window::Hour));
    let repo = PgSwapActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let bars = repo
        .list_protocol_tranches(&period, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let previous = repo
        .find_protocol_totals_in_tranches(&before, None)
        .await
        .unwrap();

    assert_eq!(bars.len(), 12);
    assert_eq!(bars[0].from, minutes_before(60));
    assert_eq!(bars[11].value.swaps, 2);
    assert_eq!(bars[5].value.swaps, 0);
    assert_eq!(bars[5].value.volume_usd, Some(Decimal::ZERO));
    assert_eq!(previous[0].activity.swaps, 3);
}

/// Against a token's pools counted per tranche, its hour cut at 11 or 13 tranches, a page cut
/// before it is ranked, or a cursor not followed: the token of two pools has two pools and their
/// volume within the hour, and pages of one list every token once, in order, ties included.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hours_tokens_count_their_pools_once(pool: PgPool) {
    write_tranches(&pool).await;
    let (x, y, shared) = (pk(1).to_string(), pk(2).to_string(), pk(12).to_string());
    seed_pool(&pool, &x, &shared, &pk(10).to_string()).await;
    seed_pool(&pool, &y, &shared, &pk(11).to_string()).await;
    tranche(&pool, &x, minutes_before(60), 1, Some(10)).await;
    tranche(&pool, &x, minutes_before(10), 1, Some(10)).await;
    tranche(&pool, &y, minutes_before(20), 1, Some(5)).await;
    tranche(&pool, &y, minutes_before(65), 1, Some(1000)).await;
    tranche(&pool, &y, through(), 1, Some(1000)).await;
    // Two tokens of one pool tie on its volume: only the mint orders them.
    let z = pk(3).to_string();
    let mut tied = [pk(13).to_string(), pk(14).to_string()];
    seed_pool(&pool, &z, &tied[0], &tied[1]).await;
    tranche(&pool, &z, minutes_before(15), 1, Some(7)).await;
    tied.sort();

    let period = last_hour(&pool).await;
    let repo = PgTokenActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let ranking = repo.list_in_tranches(&period, None, 10).await.unwrap();

    assert_eq!(ranking.last_hour, period.as_of);
    let first = &ranking.page.items[0];
    assert_eq!(first.mint.to_string(), shared);
    assert_eq!(first.volume_usd, Decimal::from(25));
    assert_eq!((first.pools, first.pool_hours), (2, 3));
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = repo
            .list_in_tranches(&period, cursor.take(), 1)
            .await
            .unwrap();
        seen.extend(page.page.items.iter().map(|t| t.mint.to_string()));
        // ⚠️ A cursor that rereads its own row would page forever: fail instead of hanging.
        assert!(seen.len() <= 10, "paging does not end: {seen:?}");
        match page.page.next_cursor {
            Some(Cursor::TokenActivity(next)) => cursor = Some(next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }
    assert_eq!(
        seen,
        [
            shared,
            pk(10).to_string(),
            tied[0].clone(),
            tied[1].clone(),
            pk(11).to_string()
        ]
    );
}

/// Against the coverage taken from the tranches recorded rather than those still held: past their
/// retention, nothing is covered.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hour_is_covered_from_the_first_tranche_still_held(pool: PgPool) {
    write_tranches(&pool).await;
    let repo =
        PgProtocolActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let none_held = repo.written_tranches().await.unwrap();
    let x = pk(1).to_string();
    seed_pool(&pool, &x, &pk(10).to_string(), &pk(11).to_string()).await;
    tranche(&pool, &x, minutes_before(65), 1, Some(10)).await;
    let tranches = repo.written_tranches().await.unwrap().unwrap();

    assert_eq!(none_held, None);
    assert_eq!(tranches.first, minutes_before(65));
    assert_eq!(tranches.through, through());
}

async fn initialize(pool: &PgPool, address: &str, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_initialize_pool_events
           (pool_address, signature, token_a_mint, token_b_mint, creator, payer, alpha_vault,
            sqrt_min_price, sqrt_max_price, sqrt_price, liquidity,
            activation_type, activation_point, collect_fee_mode, pool_type,
            token_a_flag, token_b_flag, token_a_amount, token_b_amount,
            total_amount_a, total_amount_b, pool_fees_raw, timestamp, slot, event_index)
         VALUES ($1, $1, 'a', 'b', 'c', 'p', 'v', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                 '\\x00', $2, 0, 0)",
    )
    .bind(address)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// Against the hour's creations counted outside it, or for a protocol without a creation table:
/// two creations in the hour, one before it, and none for DLMM.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hours_creations_are_its_own(pool: PgPool) {
    write_tranches(&pool).await;
    let held = pk(9).to_string();
    seed_pool(&pool, &held, &pk(10).to_string(), &pk(11).to_string()).await;
    tranche(&pool, &held, minutes_before(120), 1, Some(1)).await;
    initialize(&pool, &pk(1).to_string(), minutes_before(70)).await;
    initialize(&pool, &pk(2).to_string(), minutes_before(40)).await;
    initialize(&pool, &pk(3).to_string(), minutes_before(1)).await;

    let period = last_hour(&pool).await;
    let repo = PgPoolCreationsRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let totals = repo.list_totals_in_tranches(&period, None).await.unwrap();
    let dlmm = repo
        .list_totals_in_tranches(&period, Some(Protocol::MeteoraDlmm))
        .await
        .unwrap();
    let bars = repo.list_tranches(&period, None).await.unwrap();

    assert_eq!(totals.len(), 1);
    assert_eq!(totals[0].pools, 2);
    assert!(dlmm.is_empty());
    assert_eq!(bars.len(), 12);
    assert_eq!(bars.iter().map(|b| b.value.pools).sum::<i64>(), 2);
}

/// A tranche held two hours back: the last hour has a coverage to read from.
async fn hold_a_tranche(pool: &PgPool) {
    let held = pk(9).to_string();
    seed_pool(pool, &held, &pk(10).to_string(), &pk(11).to_string()).await;
    tranche(pool, &held, minutes_before(120), 1, Some(1)).await;
}

async fn api(pool: &PgPool) -> PgPool {
    setup_roles_and_connect_as(pool, "yog_api").await
}

async fn liquidity_move(
    pool: &PgPool,
    sig: &str,
    at: DateTime<Utc>,
    owner: u8,
    kind: &str,
    usd: Option<i64>,
) {
    sqlx::query(
        "INSERT INTO analytic_liquidity_moves
           (signature, event_index, timestamp, bucket, pool_address, protocol, owner, kind,
            amount_usd)
         VALUES ($1, 0, $2, time_bucket('5 minutes', $2), $6, 'meteora_damm_v2', $3, $4, $5)",
    )
    .bind(sig)
    .bind(at)
    .bind(pk(owner).to_string())
    .bind(kind)
    .bind(usd.map(BigDecimal::from))
    .bind(pk(20).to_string())
    .execute(pool)
    .await
    .unwrap();
}

/// Against the moves' hour cut at 11 or 13 tranches, or their owners counted per move: the move
/// just before the hour and the one at its end stay out, an owner of two moves counts once.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hours_moves_are_its_own_and_their_owners_distinct(pool: PgPool) {
    write_tranches(&pool).await;
    hold_a_tranche(&pool).await;
    liquidity_move(&pool, "before", minutes_before(65), 1, "add", Some(1000)).await;
    liquidity_move(&pool, "first", minutes_before(60), 1, "add", Some(10)).await;
    liquidity_move(&pool, "mid", minutes_before(30), 1, "create", Some(5)).await;
    liquidity_move(&pool, "unvalued", minutes_before(10), 2, "remove", None).await;
    liquidity_move(&pool, "after", through(), 3, "add", Some(1000)).await;

    let period = last_hour(&pool).await;
    let repo = PgLiquidityMoveRepository::new(api(&pool).await);
    let totals = repo
        .find_totals_in_tranches(&period, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let bars = repo
        .list_tranches(&period, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(
        (
            totals.flows.added_usd,
            totals.flows.created_usd,
            totals.flows.removed_usd
        ),
        (Decimal::from(10), Decimal::from(5), Decimal::ZERO)
    );
    assert_eq!(
        (totals.flows.moves, totals.flows.moves_valued, totals.owners),
        (3, 2, 2)
    );
    assert_eq!(bars.len(), 12);
    assert_eq!(bars[0].value.added_usd, Decimal::from(10));
    assert_eq!(bars[6].value.created_usd, Decimal::from(5));
    assert_eq!(bars.iter().map(|b| b.value.moves).sum::<i64>(), 3);
}

/// Against the list of the hour read past its ends or paged with a repeat: the move just before
/// the hour and the one at its end stay out, the unvalued too, each other move listed once.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hours_list_of_moves_is_its_own(pool: PgPool) {
    write_tranches(&pool).await;
    hold_a_tranche(&pool).await;
    liquidity_move(&pool, "before", minutes_before(65), 1, "add", Some(1000)).await;
    liquidity_move(&pool, "first", minutes_before(60), 1, "add", Some(10)).await;
    liquidity_move(&pool, "mid", minutes_before(30), 1, "create", Some(5)).await;
    liquidity_move(&pool, "unvalued", minutes_before(10), 2, "remove", None).await;
    liquidity_move(&pool, "after", through(), 3, "add", Some(1000)).await;

    let period = last_hour(&pool).await;
    let repo = PgLiquidityMoveRepository::new(api(&pool).await);
    let list = LiquidityMoveList::every_kind(LiquidityMoveScope::Protocol(Protocol::MeteoraDammV2));
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..5 {
        let page = repo
            .list_moves_in_tranches(
                &period,
                Window::Hour,
                list,
                LiquidityMoveOrder::Amount,
                cursor.take(),
                1,
            )
            .await
            .unwrap();
        seen.extend(page.items.iter().map(|m| m.signature.clone()));
        match page.next_cursor {
            Some(Cursor::LiquidityMove(next)) => cursor = Some(*next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }

    assert_eq!(seen, ["first", "mid"]);
}

async fn claim(
    pool: &PgPool,
    kind: ClaimKind,
    sig: &str,
    at: DateTime<Utc>,
    owner: u8,
    usd: Option<i64>,
) {
    let sql = match kind {
        ClaimKind::Reward => {
            "INSERT INTO analytic_reward_claims (signature, event_index, timestamp, bucket,
               pool_address, protocol, owner, amount_usd, mint_reward)
             VALUES ($1, 0, $2, time_bucket('5 minutes', $2), 'pool', 'meteora_damm_v2', $3, $4, 'mint')"
        }
        ClaimKind::PositionFee => {
            "INSERT INTO analytic_position_fee_claims (signature, event_index, timestamp, bucket,
               pool_address, protocol, owner, amount_usd)
             VALUES ($1, 0, $2, time_bucket('5 minutes', $2), 'pool', 'meteora_damm_v2', $3, $4)"
        }
    };
    sqlx::query(sql)
        .bind(sig)
        .bind(at)
        .bind(pk(owner).to_string())
        .bind(usd.map(BigDecimal::from))
        .execute(pool)
        .await
        .unwrap();
}

fn frozen(from: DateTime<Utc>, as_of: DateTime<Utc>, covered_since: DateTime<Utc>) -> FrozenPeriod {
    FrozenPeriod {
        from,
        as_of,
        covered_since,
    }
}

/// Against the claims' kinds crossed, their hour cut at 11 or 13 tranches, or a bucket summed
/// partial before the coverage: each series and total reads its own claims within its bounds.
#[sqlx::test(migrations = "./migrations")]
async fn the_claims_are_read_by_tranche_by_hour_and_by_day(pool: PgPool) {
    write_tranches(&pool).await;
    hold_a_tranche(&pool).await;
    claim(
        &pool,
        ClaimKind::PositionFee,
        "a",
        minutes_before(65),
        1,
        Some(1),
    )
    .await;
    claim(
        &pool,
        ClaimKind::PositionFee,
        "b",
        minutes_before(55),
        1,
        Some(3),
    )
    .await;
    claim(
        &pool,
        ClaimKind::PositionFee,
        "c",
        minutes_before(20),
        1,
        None,
    )
    .await;
    claim(&pool, ClaimKind::PositionFee, "d", through(), 2, Some(1000)).await;
    claim(
        &pool,
        ClaimKind::Reward,
        "r",
        minutes_before(40),
        3,
        Some(7),
    )
    .await;

    let period = last_hour(&pool).await;
    let repo = PgClaimTotalsRepository::new(api(&pool).await);
    let damm = Protocol::MeteoraDammV2;
    let fees = repo
        .find_totals_in_tranches(ClaimKind::PositionFee, &period, damm)
        .await
        .unwrap();
    let rewards = repo
        .find_totals_in_tranches(ClaimKind::Reward, &period, damm)
        .await
        .unwrap();
    let bars = repo
        .list_tranches(ClaimKind::PositionFee, &period, damm)
        .await
        .unwrap();
    let noon = Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap();
    let hours = repo
        .list_hourly(
            ClaimKind::PositionFee,
            &frozen(noon - TimeDelta::hours(3), noon, noon - TimeDelta::hours(3)),
            damm,
        )
        .await
        .unwrap();
    let day = |covered| frozen(noon - TimeDelta::days(7), noon, covered);
    let whole = repo
        .list_daily(
            ClaimKind::PositionFee,
            &day(noon - TimeDelta::days(1)),
            damm,
        )
        .await
        .unwrap();
    let partial = repo
        .list_daily(
            ClaimKind::PositionFee,
            &day(noon - TimeDelta::hours(23)),
            damm,
        )
        .await
        .unwrap();

    assert_eq!((fees.claims, fees.claims_valued, fees.owners), (2, 1, 1));
    assert_eq!(fees.amount_usd, Decimal::from(3));
    assert_eq!((rewards.claims, rewards.amount_usd), (1, Decimal::from(7)));
    assert_eq!(bars.len(), 12);
    assert_eq!(bars.iter().map(|b| b.value.claims).sum::<i64>(), 2);
    assert_eq!(hours.len(), 3);
    assert_eq!((hours[2].value.claims, hours[2].value.owners), (3, 1));
    assert_eq!(whole.len(), 1);
    assert_eq!(whole[0].value.claims, 3);
    assert!(partial.is_empty());
}

/// A position opened, or closed, at `at`.
async fn position_event(pool: &PgPool, opened: bool, sig: &str, at: DateTime<Utc>) {
    let sql = if opened {
        "INSERT INTO meteora_damm_v2_create_position_events
           (pool_address, signature, owner, position, position_nft_mint, timestamp, slot,
            event_index)
         VALUES ('x', $1, 'x', 'x', 'x', $2, 0, 0)"
    } else {
        "INSERT INTO meteora_damm_v2_close_position_events
           (pool_address, signature, owner, position, position_nft_mint, timestamp, slot,
            event_index)
         VALUES ('x', $1, 'x', 'x', 'x', $2, 0, 0)"
    };
    sqlx::query(sql)
        .bind(sig)
        .bind(at)
        .execute(pool)
        .await
        .unwrap();
}

/// Against the hour's changes counted outside it, or a frozen bucket without any hour written read
/// as 0: the last hour counts its events, the hours their own rows only.
#[sqlx::test(migrations = "./migrations")]
async fn the_position_changes_are_counted_by_tranche_and_by_hour(pool: PgPool) {
    write_tranches(&pool).await;
    hold_a_tranche(&pool).await;
    position_event(&pool, true, "before", minutes_before(65)).await;
    position_event(&pool, true, "in", minutes_before(50)).await;
    position_event(&pool, false, "out", minutes_before(10)).await;
    position_event(&pool, true, "after", through()).await;
    sqlx::query(
        "INSERT INTO analytic_protocol_position_changes_hourly
           (protocol, bucket, opened, vesting_locks, permanent_locks, splits, closed)
         VALUES ('meteora_damm_v2', $1, 2, 0, 0, 0, 1), ('meteora_damm_v2', $2, 3, 0, 0, 0, 0)",
    )
    .bind(Utc.with_ymd_and_hms(2026, 10, 20, 10, 0, 0).unwrap())
    .bind(Utc.with_ymd_and_hms(2026, 10, 20, 11, 0, 0).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let period = last_hour(&pool).await;
    let repo = PgPositionChangeCountsRepository::new(api(&pool).await);
    let totals = repo
        .find_totals_in_tranches(&period, Protocol::MeteoraDammV2)
        .await
        .unwrap()
        .unwrap();
    let bars = repo
        .list_tranches(&period, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let noon = Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap();
    let three = noon - TimeDelta::hours(3);
    let hours = repo
        .list_hourly(&frozen(three, noon, three), Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!((totals.opened, totals.closed), (1, 1));
    assert_eq!(bars.len(), 12);
    assert_eq!(hours.len(), 2);
    assert_eq!((hours[0].value.opened, hours[1].value.opened), (2, 3));
}

/// Against a pool's hour cut at 11 or 13 tranches, or a pool seen and idle read as unknown: its
/// swaps of the hour, 0 for a pool seen before it that did not swap, nothing for one never seen.
#[sqlx::test(migrations = "./migrations")]
async fn a_pools_last_hour_is_its_twelve_tranches(pool: PgPool) {
    write_tranches(&pool).await;
    let x = pk(1).to_string();
    seed_pool(&pool, &x, &pk(10).to_string(), &pk(11).to_string()).await;
    tranche(&pool, &x, minutes_before(65), 100, Some(100)).await;
    tranche(&pool, &x, minutes_before(60), 1, Some(10)).await;
    tranche(&pool, &x, minutes_before(5), 2, Some(20)).await;
    tranche(&pool, &x, through(), 100, Some(100)).await;
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint, first_seen_at)
         VALUES ($1, 'meteora_damm_v2', $2, $3, $4)",
    )
    .bind(pk(4).to_string())
    .bind(pk(10).to_string())
    .bind(pk(12).to_string())
    .bind(minutes_before(120))
    .execute(&pool)
    .await
    .unwrap();

    let period = last_hour(&pool).await;
    let repo = PgSwapActivityRepository::new(api(&pool).await);
    let totals = repo
        .find_pool_totals_in_tranches(&period, pk(1))
        .await
        .unwrap()
        .unwrap();
    let bars = repo.list_pool_tranches(&period, pk(1)).await.unwrap();
    let idle = repo
        .find_pool_totals_in_tranches(&period, pk(4))
        .await
        .unwrap()
        .unwrap();
    let never = repo
        .find_pool_totals_in_tranches(&period, pk(3))
        .await
        .unwrap();

    assert_eq!(totals.swaps, 3);
    assert_eq!(totals.volume_usd, Some(Decimal::from(30)));
    assert_eq!(bars.len(), 12);
    assert_eq!((bars[0].activity.swaps, bars[11].activity.swaps), (1, 2));
    assert_eq!((idle.swaps, idle.volume_usd), (0, Some(Decimal::ZERO)));
    assert!(never.is_none());
}

/// Against a run's bounds read inclusive at its end, or another pool's tranches counted in it:
/// a tranche is in a run from its start to its end, of its own pool only.
#[sqlx::test(migrations = "./migrations")]
async fn the_last_hours_volume_is_split_by_the_runs_tranche_by_tranche(pool: PgPool) {
    write_tranches(&pool).await;
    let (x, y) = (pk(1).to_string(), pk(2).to_string());
    seed_pool(&pool, &x, &pk(10).to_string(), &pk(11).to_string()).await;
    seed_pool(&pool, &y, &pk(10).to_string(), &pk(12).to_string()).await;
    for minutes in [60, 55, 50, 45] {
        tranche(&pool, &x, minutes_before(minutes), 1, Some(10)).await;
    }
    tranche(&pool, &y, minutes_before(55), 1, Some(5)).await;
    sqlx::query(
        "INSERT INTO analytic_rotation_runs (pool_address, started_at, protocol, ended_at,
           volume_usd, first_hour_volume_usd, extinct, settled)
         VALUES ($1, $2, 'meteora_damm_v2', $3, 20, 20, false, false)",
    )
    .bind(&x)
    .bind(minutes_before(55))
    .bind(minutes_before(45))
    .execute(&pool)
    .await
    .unwrap();

    let period = last_hour(&pool).await;
    let repo = PgRotationRunRepository::new(api(&pool).await);
    let bars = repo
        .list_tranches(&period, Some(minutes_before(120)), Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let split = |i: usize| (bars[i].value.volume_usd, bars[i].value.in_runs_usd);

    assert_eq!(bars.len(), 12);
    assert_eq!(split(0), (Some(Decimal::from(10)), Some(Decimal::ZERO)));
    assert_eq!(split(1), (Some(Decimal::from(15)), Some(Decimal::from(10))));
    assert_eq!(split(2), (Some(Decimal::from(10)), Some(Decimal::from(10))));
    assert_eq!(split(3), (Some(Decimal::from(10)), Some(Decimal::ZERO)));
    assert_eq!(split(4), (Some(Decimal::ZERO), Some(Decimal::ZERO)));
}
