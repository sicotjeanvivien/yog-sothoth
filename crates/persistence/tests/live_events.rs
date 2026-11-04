//! The live reads of the event collections under `yog_api`, played on each of their tables: by
//! id and not by time, within the lookback, capped.

use std::fmt::Debug;

use chrono::Duration;
use solana_signature::Signature;
use sqlx::{AssertSqlSafe, PgPool};
use yog_core::RepositoryResult;
use yog_core::domain::{
    EventTable, LiquidityChangeTable, LiveEventFeed, LiveRecord, PoolChangeTable,
    PoolCreationTable, PositionChangeTable, PositionFeeClaimTable, ProtocolFeeClaimTable,
    RewardChangeTable, RewardClaimTable,
};
use yog_persistence::{
    PgLiquidityChangeEventRepository, PgPoolChangeEventRepository, PgPoolCreationEventRepository,
    PgPositionChangeEventRepository, PgPositionFeeClaimEventRepository,
    PgProtocolFeeClaimEventRepository, PgRewardChangeEventRepository, PgRewardClaimEventRepository,
};

use super::helpers::{pk, setup_roles_and_connect_as};

/// An event of one table, `$1` its pool, `$2` its signature, `$3` its instant, `$4` its index,
/// and `$5` a key, when the table has one.
struct Insert {
    sql: &'static str,
    has_key: bool,
}

const fn with_key(sql: &'static str) -> Insert {
    Insert { sql, has_key: true }
}

const fn without_key(sql: &'static str) -> Insert {
    Insert {
        sql,
        has_key: false,
    }
}

/// Inserts one event stamped `ago` seconds back, and returns its id.
async fn insert(pool: &PgPool, insert: &Insert, seed: u8, ago: i64) -> i64 {
    let sql = format!("{} RETURNING id", insert.sql);
    let query = sqlx::query_scalar::<_, i64>(AssertSqlSafe(sql))
        .bind(pk(1).to_string())
        .bind(Signature::from([seed; 64]).to_string())
        .bind(chrono::Utc::now() - Duration::seconds(ago))
        .bind(0_i32);
    let query = if insert.has_key {
        query.bind(pk(50).to_string())
    } else {
        query
    };
    query.fetch_one(pool).await.unwrap()
}

/// Mutation: a read ordered or bounded by the instant, a lookback or the limit dropped — the late
/// event missed, the batch cut out of id order, the stale event read, or the tail read whole.
async fn reads_by_id_within_the_lookback<T: EventTable + Debug, E>(
    admin: &PgPool,
    feed: &dyn LiveEventFeed<T, E>,
    table: T,
    sql: &Insert,
) {
    let first = insert(admin, sql, 1, 10).await;
    let late = insert(admin, sql, 2, 60).await;
    let newest = insert(admin, sql, 3, 5).await;
    insert(admin, sql, 4, 2 * 86_400).await;
    let lookback = Duration::hours(24);
    let ids = |records: Vec<LiveRecord<RepositoryResult<E>>>| {
        records
            .into_iter()
            .map(|r| {
                assert!(r.event.is_ok(), "{table:?}: event {} decodes", r.id);
                r.id
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(
        feed.live_tip(table, lookback).await.unwrap(),
        Some(newest),
        "{table:?}: the tip leaves the stale event out"
    );
    assert_eq!(
        ids(feed.live_after(table, first, lookback, 200).await.unwrap()),
        vec![late, newest],
        "{table:?}: after the first, the one inserted after it but stamped before, and not the stale"
    );
    assert_eq!(
        ids(feed.live_after(table, 0, lookback, 2).await.unwrap()),
        vec![first, late],
        "{table:?}: capped at the limit, by id and not by instant"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn liquidity_changes_read_live(pool: PgPool) {
    let feed =
        PgLiquidityChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let sql = with_key(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind, amount_a, amount_b, liquidity_delta,
            reserve_a_after, reserve_b_after, position, owner, timestamp, slot, event_index,
            transaction_index)
         VALUES ($1, $2, 'add', 1, 2, 3, 3, 4, $5, $5, $3, 9, $4, NULL)",
    );
    reads_by_id_within_the_lookback(&pool, &feed, LiquidityChangeTable::MeteoraDammV2, &sql).await;
}

/// Mutation: the batch decoded as a whole — one bad row fails the read, and its table streams
/// nothing until the row leaves the lookback.
#[sqlx::test(migrations = "./migrations")]
async fn a_row_that_does_not_decode_comes_as_its_error_and_the_next_still_comes(pool: PgPool) {
    let feed =
        PgLiquidityChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let sql = "INSERT INTO meteora_damm_v2_liquidity_events
                 (pool_address, signature, liquidity_event_kind, amount_a, amount_b,
                  liquidity_delta, reserve_a_after, reserve_b_after, position, owner, timestamp,
                  slot, event_index, transaction_index)
               VALUES ($1, $2, 'add', 1, 2, 3, 3, 4, $3, $3, now(), 9, 0, NULL)
               RETURNING id";
    let mut ids = Vec::new();
    for (seed, address) in [(1, "not-a-pubkey".to_string()), (2, pk(1).to_string())] {
        let id: i64 = sqlx::query_scalar(sql)
            .bind(address)
            .bind(Signature::from([seed; 64]).to_string())
            .bind(pk(50).to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
        ids.push(id);
    }

    let records = feed
        .live_after(
            LiquidityChangeTable::MeteoraDammV2,
            0,
            Duration::hours(24),
            200,
        )
        .await
        .expect("one bad row does not fail the read");

    assert_eq!(records.iter().map(|r| r.id).collect::<Vec<_>>(), ids);
    assert!(records[0].event.is_err(), "the bad row comes as its error");
    assert!(records[1].event.is_ok());
}

#[sqlx::test(migrations = "./migrations")]
async fn pool_creations_read_live(pool: PgPool) {
    let feed =
        PgPoolCreationEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let sql = with_key(
        "INSERT INTO meteora_damm_v2_initialize_pool_events
           (pool_address, signature, token_a_mint, token_b_mint, creator, payer, alpha_vault,
            sqrt_min_price, sqrt_max_price, sqrt_price, liquidity, activation_type,
            activation_point, collect_fee_mode, pool_type, token_a_flag, token_b_flag,
            token_a_amount, token_b_amount, total_amount_a, total_amount_b, pool_fees_raw,
            timestamp, slot, event_index, transaction_index)
         VALUES ($1, $2, $5, $5, $5, $5, $5, 1, 2, 3, 4, 1, 5, 0, 0, 0, 0, 6, 7, 8, 9,
                 '\\x00', $3, 9, $4, NULL)",
    );
    reads_by_id_within_the_lookback(&pool, &feed, PoolCreationTable::MeteoraDammV2, &sql).await;
}

#[sqlx::test(migrations = "./migrations")]
async fn position_fee_claims_read_live(pool: PgPool) {
    let feed =
        PgPositionFeeClaimEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let sql = with_key(
        "INSERT INTO meteora_damm_v2_claim_position_fee_events
           (pool_address, signature, position, owner, fee_a_claimed, fee_b_claimed, timestamp,
            slot, event_index, transaction_index)
         VALUES ($1, $2, $5, $5, 1, 2, $3, 9, $4, NULL)",
    );
    reads_by_id_within_the_lookback(&pool, &feed, PositionFeeClaimTable::MeteoraDammV2, &sql).await;
}

#[sqlx::test(migrations = "./migrations")]
async fn protocol_fee_claims_read_live(pool: PgPool) {
    let feed =
        PgProtocolFeeClaimEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let sql = without_key(
        "INSERT INTO meteora_damm_v2_claim_protocol_fee_events
           (pool_address, signature, token_a_amount, token_b_amount, timestamp, slot,
            event_index, transaction_index)
         VALUES ($1, $2, 1, 2, $3, 9, $4, NULL)",
    );
    reads_by_id_within_the_lookback(&pool, &feed, ProtocolFeeClaimTable::MeteoraDammV2, &sql).await;
}

#[sqlx::test(migrations = "./migrations")]
async fn reward_claims_read_live(pool: PgPool) {
    let feed =
        PgRewardClaimEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let sql = with_key(
        "INSERT INTO meteora_damm_v2_claim_reward_events
           (pool_address, signature, position, owner, mint_reward, reward_index, total_reward,
            timestamp, slot, event_index, transaction_index)
         VALUES ($1, $2, $5, $5, $5, 1, 3, $3, 9, $4, NULL)",
    );
    reads_by_id_within_the_lookback(&pool, &feed, RewardClaimTable::MeteoraDammV2, &sql).await;
}

#[sqlx::test(migrations = "./migrations")]
async fn position_changes_read_live_table_by_table(pool: PgPool) {
    let feed =
        PgPositionChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let tables = [
        (
            PositionChangeTable::MeteoraDammV2Open,
            with_key(
                "INSERT INTO meteora_damm_v2_create_position_events
                   (pool_address, signature, owner, position, position_nft_mint, timestamp, slot,
                    event_index, transaction_index)
                 VALUES ($1, $2, $5, $5, $5, $3, 9, $4, NULL)",
            ),
        ),
        (
            PositionChangeTable::MeteoraDammV2Close,
            with_key(
                "INSERT INTO meteora_damm_v2_close_position_events
                   (pool_address, signature, owner, position, position_nft_mint, timestamp, slot,
                    event_index, transaction_index)
                 VALUES ($1, $2, $5, $5, $5, $3, 9, $4, NULL)",
            ),
        ),
        (
            PositionChangeTable::MeteoraDammV2VestingLock,
            with_key(
                "INSERT INTO meteora_damm_v2_lock_position_events
                   (pool_address, signature, position, owner, vesting, cliff_point,
                    period_frequency, cliff_unlock_liquidity, liquidity_per_period,
                    number_of_period, timestamp, slot, event_index, transaction_index)
                 VALUES ($1, $2, $5, $5, $5, 1, 2, 3, 4, 5, $3, 9, $4, NULL)",
            ),
        ),
        (
            PositionChangeTable::MeteoraDammV2PermanentLock,
            with_key(
                "INSERT INTO meteora_damm_v2_permanent_lock_position_events
                   (pool_address, signature, position, lock_liquidity_amount,
                    total_permanent_locked_liquidity, timestamp, slot, event_index,
                    transaction_index)
                 VALUES ($1, $2, $5, 1, 2, $3, 9, $4, NULL)",
            ),
        ),
        (
            PositionChangeTable::MeteoraDammV2Split,
            with_key(
                "INSERT INTO meteora_damm_v2_split_position_events
                   (pool_address, signature, first_owner, second_owner, first_position,
                    second_position, current_sqrt_price,
                    split_permanent_locked_liquidity, split_unlocked_liquidity,
                    split_vested_liquidity, split_fee_a, split_fee_b, split_reward_0,
                    split_reward_1, first_unlocked_liquidity, first_permanent_locked_liquidity,
                    first_vested_liquidity, first_fee_a, first_fee_b, first_reward_0,
                    first_reward_1, second_unlocked_liquidity, second_permanent_locked_liquidity,
                    second_vested_liquidity, second_fee_a, second_fee_b, second_reward_0,
                    second_reward_1, num_unlocked_liquidity, num_permanent_locked_liquidity,
                    num_fee_a, num_fee_b, num_reward_0, num_reward_1, num_inner_vesting_liquidity,
                    timestamp, slot, event_index, transaction_index)
                 VALUES ($1, $2, $5, $5, $5, $5, 7, 1, 2, 3, 4, 5, 6, 7, 1, 2, 3, 4, 5, 6, 7,
                         1, 2, 3, 4, 5, 6, 7, 1, 2, 3, 4, 5, 6, 7, $3, 9, $4, NULL)",
            ),
        ),
    ];
    assert_eq!(tables.len(), PositionChangeTable::ALL.len());
    for (table, sql) in &tables {
        reads_by_id_within_the_lookback(&pool, &feed, *table, sql).await;
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn reward_changes_read_live_table_by_table(pool: PgPool) {
    let feed =
        PgRewardChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let tables = [
        (
            RewardChangeTable::MeteoraDammV2Initialize,
            with_key(
                "INSERT INTO meteora_damm_v2_initialize_reward_events
                   (pool_address, signature, timestamp, slot, event_index, transaction_index,
                    reward_mint, funder, creator, reward_index, reward_duration)
                 VALUES ($1, $2, $3, 9, $4, NULL, $5, $5, $5, 0, 60)",
            ),
        ),
        (
            RewardChangeTable::MeteoraDammV2Fund,
            with_key(
                "INSERT INTO meteora_damm_v2_fund_reward_events
                   (pool_address, signature, timestamp, slot, event_index, transaction_index,
                    funder, mint_reward, reward_index, amount, transfer_fee_excluded_amount_in,
                    reward_duration_end, pre_reward_rate, post_reward_rate)
                 VALUES ($1, $2, $3, 9, $4, NULL, $5, $5, 0, 1, 1, 2, 3, 4)",
            ),
        ),
        (
            RewardChangeTable::MeteoraDammV2DurationChange,
            without_key(
                "INSERT INTO meteora_damm_v2_update_reward_duration_events
                   (pool_address, signature, timestamp, slot, event_index, transaction_index,
                    reward_index, old_reward_duration, new_reward_duration)
                 VALUES ($1, $2, $3, 9, $4, NULL, 0, 1, 2)",
            ),
        ),
        (
            RewardChangeTable::MeteoraDammV2FunderChange,
            with_key(
                "INSERT INTO meteora_damm_v2_update_reward_funder_events
                   (pool_address, signature, timestamp, slot, event_index, transaction_index,
                    reward_index, old_funder, new_funder)
                 VALUES ($1, $2, $3, 9, $4, NULL, 0, $5, $5)",
            ),
        ),
        (
            RewardChangeTable::MeteoraDammV2IneligibleWithdrawal,
            with_key(
                "INSERT INTO meteora_damm_v2_withdraw_ineligible_reward_events
                   (pool_address, signature, timestamp, slot, event_index, transaction_index,
                    reward_mint, amount)
                 VALUES ($1, $2, $3, 9, $4, NULL, $5, 0)",
            ),
        ),
        (
            RewardChangeTable::MeteoraDammV2DeadLiquidityWithdrawal,
            with_key(
                "INSERT INTO meteora_damm_v2_withdraw_dead_liquidity_reward_events
                   (pool_address, signature, timestamp, slot, event_index, transaction_index,
                    reward_mint, amount)
                 VALUES ($1, $2, $3, 9, $4, NULL, $5, 1)",
            ),
        ),
    ];
    assert_eq!(tables.len(), RewardChangeTable::ALL.len());
    for (table, sql) in &tables {
        reads_by_id_within_the_lookback(&pool, &feed, *table, sql).await;
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn pool_changes_read_live_table_by_table(pool: PgPool) {
    let feed = PgPoolChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let tables = [
        (
            PoolChangeTable::MeteoraDammV2Status,
            without_key(
                "INSERT INTO meteora_damm_v2_set_pool_status_events
                   (pool_address, signature, timestamp, slot, event_index, transaction_index,
                    status)
                 VALUES ($1, $2, $3, 9, $4, NULL, 0)",
            ),
        ),
        (
            PoolChangeTable::MeteoraDammV2Fees,
            with_key(
                "INSERT INTO meteora_damm_v2_update_pool_fees_events
                   (pool_address, signature, timestamp, slot, event_index, transaction_index,
                    operator, params_raw)
                 VALUES ($1, $2, $3, 9, $4, NULL, $5, '\\x01')",
            ),
        ),
    ];
    assert_eq!(tables.len(), PoolChangeTable::ALL.len());
    for (table, sql) in &tables {
        reads_by_id_within_the_lookback(&pool, &feed, *table, sql).await;
    }
}
