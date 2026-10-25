//! `PgLiquidityMoveRepository` under `yog_api`: a protocol's liquidity and a pool's pages of
//! moves, from the tables as yog-analytic leaves them, seeded here.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::{
    Cursor,
    domain::{FrozenPeriod, LiquidityMoveLookup, LiquidityMoveOrder, Protocol, Window},
};
use yog_persistence::PgLiquidityMoveRepository;

use super::helpers::{DAMM, DLMM, as_of, period, pk, setup_roles_and_connect_as};

async fn repository(pool: &PgPool) -> PgLiquidityMoveRepository {
    PgLiquidityMoveRepository::new(setup_roles_and_connect_as(pool, "yog_api").await)
}

async fn insert_liquidity_hour(pool: &PgPool, protocol: &str, bucket: DateTime<Utc>, added: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_liquidity_hourly
           (protocol, bucket, created_usd, added_usd, removed_usd, moves, moves_valued)
         VALUES ($1, $2, 0, $3, 0, 1, 1)",
    )
    .bind(protocol)
    .bind(bucket)
    .bind(added)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: `find_measured_since` read over every protocol, or the protocol filter of the sums
/// dropped — DLMM's earlier start and its liquidity counted.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_liquidity_starts_and_sums_on_its_own(pool: PgPool) {
    let start = as_of() - Duration::hours(10);
    for h in 0..10 {
        insert_liquidity_hour(&pool, DAMM, start + Duration::hours(h), 5).await;
    }
    insert_liquidity_hour(&pool, DLMM, as_of() - Duration::hours(20), 1000).await;
    let repository = repository(&pool).await;
    let since = repository
        .find_measured_since(Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let covered = FrozenPeriod {
        covered_since: start,
        ..period(10)
    };

    let totals = LiquidityMoveLookup::find_totals(&repository, &covered, Protocol::MeteoraDammV2)
        .await
        .unwrap()
        .unwrap();
    let hourly = LiquidityMoveLookup::list_hourly(&repository, &covered, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(since, Some(start));
    assert_eq!(totals.added_usd, Decimal::from(50));
    assert_eq!(hourly.len(), 10);
}

/// One valued move of `pool`, worth `usd`, `minutes` before [`as_of`]; `None` leaves it unvalued.
async fn insert_move(
    pool: &PgPool,
    address: &str,
    signature: &str,
    minutes: i64,
    usd: Option<i64>,
) {
    let at = as_of() - Duration::minutes(minutes);
    sqlx::query(
        "INSERT INTO analytic_liquidity_moves
           (signature, event_index, timestamp, bucket, pool_address, protocol, owner, kind,
            amount_usd)
         VALUES ($1, 0, $2, time_bucket('5 minutes', $2), $3, 'meteora_damm_v2', $4, 'add', $5)",
    )
    .bind(signature)
    .bind(at)
    .bind(address)
    .bind(pk(9).to_string())
    .bind(usd)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the cursor's key compared on the amount alone, or without the signature — a
/// tie repeated on the next page or skipped; the pool filter dropped; the unvalued kept.
#[sqlx::test(migrations = "./migrations")]
async fn a_pools_moves_page_without_repeat_nor_gap_in_each_order(pool: PgPool) {
    let (mine, other) = (pk(1).to_string(), pk(2).to_string());
    // sig2 and sig3 tie on amount and instant: only the signature orders them.
    for (i, (minutes, usd)) in [(5, 30), (15, 20), (25, 20), (25, 20), (45, 10)]
        .into_iter()
        .enumerate()
    {
        insert_move(&pool, &mine, &format!("sig{i}"), minutes, Some(usd)).await;
    }
    insert_move(&pool, &mine, "unvalued", 1, None).await;
    insert_move(&pool, &other, "other", 2, Some(1_000)).await;
    let repository = repository(&pool).await;
    let day = period(24);

    for order in [LiquidityMoveOrder::Amount, LiquidityMoveOrder::At] {
        let mut seen = Vec::new();
        let mut cursor = None;
        loop {
            let page = repository
                .list_pool_moves(&day, Window::Day, pk(1), order, cursor.take(), 3)
                .await
                .unwrap();
            seen.extend(page.items.iter().map(|m| m.signature.clone()));
            match page.next_cursor {
                Some(Cursor::LiquidityMove(next)) => cursor = Some(*next),
                Some(other) => panic!("{other:?}"),
                None => break,
            }
        }
        assert_eq!(seen, ["sig0", "sig1", "sig2", "sig3", "sig4"], "{order:?}");
    }
}

/// Mutation: the amount compared exactly, not rounded as the cursor holds it, or the cursor
/// keyed on the whole amount — the move past its rounding is repeated or skipped.
#[sqlx::test(migrations = "./migrations")]
async fn a_page_follows_a_move_whose_amount_has_more_digits_than_a_cursor(pool: PgPool) {
    let mine = pk(1).to_string();
    for (signature, minutes, usd) in [
        ("long", 30, "2.0000000000000000000000000000000000001"),
        ("seventh", 25, "2.0000004"),
        ("round", 20, "2"),
        ("small", 10, "1"),
    ] {
        sqlx::query(
            "INSERT INTO analytic_liquidity_moves
               (signature, event_index, timestamp, bucket, pool_address, protocol, owner, kind,
                amount_usd)
             VALUES ($1, 0, $2, time_bucket('5 minutes', $2), $3, 'meteora_damm_v2', $4, 'add',
                     $5::NUMERIC)",
        )
        .bind(signature)
        .bind(as_of() - Duration::minutes(minutes))
        .bind(&mine)
        .bind(pk(9).to_string())
        .bind(usd)
        .execute(&pool)
        .await
        .unwrap();
    }
    let repository = repository(&pool).await;

    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..10 {
        let page = repository
            .list_pool_moves(
                &period(24),
                Window::Day,
                pk(1),
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

    assert_eq!(seen, ["round", "seventh", "long", "small"]);
}

/// Mutation: the amount returned rounded as the cursor key — a move reads 1234.567890 here
/// and 1234.5678901234 under its protocol.
#[sqlx::test(migrations = "./migrations")]
async fn a_pools_move_keeps_its_amount_whole_whatever_its_order(pool: PgPool) {
    sqlx::query(
        "INSERT INTO analytic_liquidity_moves
           (signature, event_index, timestamp, bucket, pool_address, protocol, owner, kind,
            amount_usd)
         VALUES ('sig', 0, $1, time_bucket('5 minutes', $1), $2, 'meteora_damm_v2', $3, 'add',
                 1234.5678901234)",
    )
    .bind(as_of() - Duration::minutes(30))
    .bind(pk(1).to_string())
    .bind(pk(9).to_string())
    .execute(&pool)
    .await
    .unwrap();
    let repository = repository(&pool).await;

    for order in [LiquidityMoveOrder::Amount, LiquidityMoveOrder::At] {
        let page = repository
            .list_pool_moves(&period(24), Window::Day, pk(1), order, None, 5)
            .await
            .unwrap();
        assert_eq!(
            page.items[0].amount_usd,
            Decimal::new(12_345_678_901_234, 10),
            "{order:?}"
        );
    }
}
