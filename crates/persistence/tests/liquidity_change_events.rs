//! `PgLiquidityChangeEventRepository` under `yog_api`: the liquidity changes of a scope from the start of a window,
//! the most recent first, paged without repeat nor gap through instants shared by several.

use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page,
    domain::{EventScope, LiquidityChangeEvent, LiquidityChangeEventLookup, Protocol},
};
use yog_persistence::PgLiquidityChangeEventRepository;

use super::helpers::{pk, setup_roles_and_connect_as};

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::seconds(seconds)
}

fn sig(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

async fn insert_change(
    pool: &PgPool,
    address: Pubkey,
    signature: Signature,
    index: i32,
    seconds: i64,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind, amount_a, amount_b, liquidity_delta,
            reserve_a_after, reserve_b_after, position, owner, timestamp, slot, event_index,
            transaction_index)
         VALUES ($1, $2, 'add', 1, 2, 18446744073709551616, 3, 4, $5, $5, $3, 9, $4, NULL)",
    )
    .bind(address.to_string())
    .bind(signature.to_string())
    .bind(at(seconds))
    .bind(index)
    .bind(pk(50).to_string())
    .execute(pool)
    .await
    .unwrap();
}

fn keys(page: &Page<LiquidityChangeEvent>) -> Vec<(DateTime<Utc>, String, u16)> {
    page.items
        .iter()
        .map(|LiquidityChangeEvent::MeteoraDammV2(e)| {
            (e.timestamp, e.signature.to_string(), e.event_index)
        })
        .collect()
}

async fn read_all(
    repository: &PgLiquidityChangeEventRepository,
    scope: EventScope,
    limit: i64,
) -> Vec<(DateTime<Utc>, String, u16)> {
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = repository
            .list_events(scope, at(-60), cursor.take(), limit)
            .await
            .unwrap();
        seen.extend(keys(&page));
        match page.next_cursor {
            Some(Cursor::LiquidityChangeEvent(next)) => cursor = Some(*next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }
    seen
}

/// Mutation: the tie-break dropped, or the cursor compared with `<=` — a change sharing an
/// instant skipped, or repeated.
#[sqlx::test(migrations = "./migrations")]
async fn a_page_follows_through_shared_instants_without_repeat_nor_gap(pool: PgPool) {
    // Four changes at one instant, two of one transaction: only the signature, then the index,
    // order them, and every page boundary of two falls among them.
    for (seed, index, seconds) in [
        (1, 0, 10),
        (2, 0, 10),
        (3, 0, 10),
        (3, 1, 10),
        (4, 0, 9),
        (5, 0, 8),
        (6, 0, 8),
    ] {
        insert_change(&pool, pk(1), sig(seed), index, seconds).await;
    }
    let repository =
        PgLiquidityChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let mut expected = read_all(&repository, EventScope::All, 50).await;
    let by_two = read_all(&repository, EventScope::All, 2).await;
    let pool_by_two = read_all(&repository, EventScope::Pool(pk(1)), 2).await;

    assert_eq!(expected.len(), 7);
    assert_eq!(by_two, expected);
    assert_eq!(pool_by_two, expected);
    let sorted = {
        expected.sort_by(|a, b| b.cmp(a));
        expected
    };
    assert_eq!(by_two, sorted);
}

/// Mutation: the pool filter, the protocol dispatch or the window's start dropped — another
/// pool's change, a DLMM page, or a change before the window, listed.
#[sqlx::test(migrations = "./migrations")]
async fn a_scope_and_its_window_read_only_their_changes(pool: PgPool) {
    insert_change(&pool, pk(1), sig(1), 0, 10).await;
    insert_change(&pool, pk(2), sig(2), 0, 20).await;
    insert_change(&pool, pk(1), sig(3), 0, -120).await;
    let repository =
        PgLiquidityChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let mine = read_all(&repository, EventScope::Pool(pk(1)), 50).await;
    let all = read_all(&repository, EventScope::All, 50).await;
    let damm = read_all(
        &repository,
        EventScope::Protocol(Protocol::MeteoraDammV2),
        50,
    )
    .await;
    let dlmm = read_all(&repository, EventScope::Protocol(Protocol::MeteoraDlmm), 50).await;

    assert_eq!(mine, [(at(10), sig(1).to_string(), 0)]);
    assert_eq!(all.len(), 2);
    assert_eq!(damm, all);
    assert!(dlmm.is_empty());
}
