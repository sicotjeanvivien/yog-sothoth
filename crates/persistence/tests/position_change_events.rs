//! `PgPositionChangeEventRepository` under `yog_api`: the five kinds of position change merged
//! into one list from the start of a window, the most recent first, paged without repeat nor
//! gap through instants shared across tables.

use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page,
    domain::{
        EventScope, MeteoraDammV2PositionChange, PositionChangeEvent, PositionChangeEventLookup,
        Protocol,
    },
};
use yog_persistence::PgPositionChangeEventRepository;

use super::helpers::{pk, setup_roles_and_connect_as};

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::seconds(seconds)
}

fn sig(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Open,
    Close,
    VestingLock,
    PermanentLock,
    Split,
}

async fn insert_change(
    pool: &PgPool,
    kind: Kind,
    address: Pubkey,
    signature: Signature,
    index: i32,
    seconds: i64,
) {
    let sql = match kind {
        Kind::Open => {
            "INSERT INTO meteora_damm_v2_create_position_events
               (pool_address, signature, owner, position, position_nft_mint, timestamp, slot,
                event_index, transaction_index)
             VALUES ($1, $2, $5, $5, $5, $3, 9, $4, NULL)"
        }
        Kind::Close => {
            "INSERT INTO meteora_damm_v2_close_position_events
               (pool_address, signature, owner, position, position_nft_mint, timestamp, slot,
                event_index, transaction_index)
             VALUES ($1, $2, $5, $5, $5, $3, 9, $4, NULL)"
        }
        Kind::VestingLock => {
            "INSERT INTO meteora_damm_v2_lock_position_events
               (pool_address, signature, position, owner, vesting, cliff_point, period_frequency,
                cliff_unlock_liquidity, liquidity_per_period, number_of_period, timestamp, slot,
                event_index, transaction_index)
             VALUES ($1, $2, $5, $5, $5, 1, 2, 18446744073709551616, 4, 5, $3, 9, $4, NULL)"
        }
        Kind::PermanentLock => {
            "INSERT INTO meteora_damm_v2_permanent_lock_position_events
               (pool_address, signature, position, lock_liquidity_amount,
                total_permanent_locked_liquidity, timestamp, slot, event_index,
                transaction_index)
             VALUES ($1, $2, $5, 1, 2, $3, 9, $4, NULL)"
        }
        Kind::Split => {
            "INSERT INTO meteora_damm_v2_split_position_events
               (pool_address, signature, first_owner, second_owner, first_position,
                second_position, current_sqrt_price,
                split_permanent_locked_liquidity, split_unlocked_liquidity,
                split_vested_liquidity, split_fee_a, split_fee_b, split_reward_0, split_reward_1,
                first_unlocked_liquidity, first_permanent_locked_liquidity,
                first_vested_liquidity, first_fee_a, first_fee_b, first_reward_0,
                first_reward_1, second_unlocked_liquidity, second_permanent_locked_liquidity,
                second_vested_liquidity, second_fee_a, second_fee_b, second_reward_0,
                second_reward_1, num_unlocked_liquidity, num_permanent_locked_liquidity,
                num_fee_a, num_fee_b, num_reward_0, num_reward_1, num_inner_vesting_liquidity,
                timestamp, slot, event_index, transaction_index)
             VALUES ($1, $2, $5, $5, $5, $5, 7, 1, 2, 3, 4, 5, 6, 7, 1, 2, 3, 4, 5, 6, 7,
                     1, 2, 3, 4, 5, 6, 7, 1, 2, 3, 4, 5, 6, 7, $3, 9, $4, NULL)"
        }
    };
    sqlx::query(sql)
        .bind(address.to_string())
        .bind(signature.to_string())
        .bind(at(seconds))
        .bind(index)
        .bind(pk(50).to_string())
        .execute(pool)
        .await
        .unwrap();
}

fn kind_of(change: &MeteoraDammV2PositionChange) -> Kind {
    match change {
        MeteoraDammV2PositionChange::Open(_) => Kind::Open,
        MeteoraDammV2PositionChange::Close(_) => Kind::Close,
        MeteoraDammV2PositionChange::VestingLock(_) => Kind::VestingLock,
        MeteoraDammV2PositionChange::PermanentLock(_) => Kind::PermanentLock,
        MeteoraDammV2PositionChange::Split(_) => Kind::Split,
    }
}

type Key = (DateTime<Utc>, String, u16, Kind);

fn keys(page: &Page<PositionChangeEvent>) -> Vec<Key> {
    page.items
        .iter()
        .map(|PositionChangeEvent::MeteoraDammV2(change)| {
            let (at, signature, index) = change.key();
            (at, signature.to_string(), index, kind_of(change))
        })
        .collect()
}

async fn read_all(
    repository: &PgPositionChangeEventRepository,
    scope: EventScope,
    limit: i64,
) -> Vec<Key> {
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = repository
            .list_events(scope, at(-60), cursor.take(), limit)
            .await
            .unwrap();
        seen.extend(keys(&page));
        match page.next_cursor {
            Some(Cursor::PositionChangeEvent(next)) => cursor = Some(*next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }
    seen
}

/// Mutation: the tie-break dropped, the cursor compared with `<=`, one table's branch cut, or
/// the rows put back in another order than the keys' — a change skipped, repeated or moved.
/// The order is the database's, its collation's, never Rust's.
#[sqlx::test(migrations = "./migrations")]
async fn a_page_follows_through_shared_instants_across_the_tables(pool: PgPool) {
    // Five changes of five tables at one instant, two of them one transaction: every page
    // boundary of two falls among them, and inside the merge of several tables.
    for (kind, seed, index, seconds) in [
        (Kind::Open, 3, 0, 10),
        (Kind::VestingLock, 3, 1, 10),
        (Kind::Close, 1, 0, 10),
        (Kind::PermanentLock, 2, 0, 10),
        (Kind::Split, 4, 0, 10),
        (Kind::Open, 5, 0, 9),
        (Kind::Close, 6, 0, 8),
    ] {
        insert_change(&pool, kind, pk(1), sig(seed), index, seconds).await;
    }
    let repository =
        PgPositionChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let expected = read_all(&repository, EventScope::All, 50).await;
    let by_two = read_all(&repository, EventScope::All, 2).await;
    let pool_by_two = read_all(&repository, EventScope::Pool(pk(1)), 2).await;

    assert_eq!(expected.len(), 7);
    assert_eq!(by_two, expected);
    assert_eq!(pool_by_two, expected);
    assert!(
        expected.windows(2).all(|w| w[0].0 >= w[1].0),
        "{expected:?}"
    );
    let one_transaction: Vec<_> = expected
        .iter()
        .filter(|k| k.1 == sig(3).to_string())
        .map(|k| (k.2, k.3))
        .collect();
    assert_eq!(one_transaction, [(1, Kind::VestingLock), (0, Kind::Open)]);
    for kind in [
        Kind::Open,
        Kind::Close,
        Kind::VestingLock,
        Kind::PermanentLock,
        Kind::Split,
    ] {
        assert!(expected.iter().any(|k| k.3 == kind), "{kind:?}");
    }
}

/// Mutation: the pool filter of one table, the protocol dispatch or the window's start dropped
/// — another pool's change, a DLMM page, or a change before the window, listed.
#[sqlx::test(migrations = "./migrations")]
async fn a_scope_and_its_window_read_only_their_changes(pool: PgPool) {
    for kind in [
        Kind::Open,
        Kind::Close,
        Kind::VestingLock,
        Kind::PermanentLock,
        Kind::Split,
    ] {
        let seed = kind as u8;
        insert_change(&pool, kind, pk(1), sig(10 + seed), 0, 10).await;
        insert_change(&pool, kind, pk(2), sig(20 + seed), 0, 20).await;
        insert_change(&pool, kind, pk(1), sig(30 + seed), 0, -120).await;
    }
    let repository =
        PgPositionChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let mine = read_all(&repository, EventScope::Pool(pk(1)), 50).await;
    let all = read_all(&repository, EventScope::All, 50).await;
    let damm = read_all(
        &repository,
        EventScope::Protocol(Protocol::MeteoraDammV2),
        50,
    )
    .await;
    let dlmm = read_all(&repository, EventScope::Protocol(Protocol::MeteoraDlmm), 50).await;

    assert_eq!(mine.len(), 5);
    assert!(mine.iter().all(|k| k.0 == at(10)), "{mine:?}");
    assert_eq!(all.len(), 10);
    assert_eq!(damm, all);
    assert!(dlmm.is_empty());
}

/// Mutation: a column of a table read into another field — the split's state after, or the
/// vesting lock's liquidity, comes back other than written.
#[sqlx::test(migrations = "./migrations")]
async fn each_kind_comes_back_as_written(pool: PgPool) {
    insert_change(&pool, Kind::VestingLock, pk(1), sig(1), 0, 10).await;
    insert_change(&pool, Kind::Split, pk(1), sig(2), 0, 9).await;
    let repository =
        PgPositionChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let page = repository
        .list_events(EventScope::All, at(-60), None, 50)
        .await
        .unwrap();

    let [
        PositionChangeEvent::MeteoraDammV2(MeteoraDammV2PositionChange::VestingLock(lock)),
        PositionChangeEvent::MeteoraDammV2(MeteoraDammV2PositionChange::Split(split)),
    ] = page.items.as_slice()
    else {
        panic!("{:?}", page.items);
    };
    assert_eq!(
        (
            lock.cliff_point,
            lock.period_frequency,
            lock.cliff_unlock_liquidity,
            lock.liquidity_per_period,
            lock.number_of_period
        ),
        (1, 2, 1 << 64, 4, 5)
    );
    assert_eq!(split.current_sqrt_price, 7);
    let a = &split.amounts;
    assert_eq!(
        (
            a.permanent_locked_liquidity,
            a.unlocked_liquidity,
            a.vested_liquidity,
            a.fee_a,
            a.fee_b,
            a.reward_0,
            a.reward_1
        ),
        (1, 2, 3, 4, 5, 6, 7)
    );
    let f = &split.first_position_after;
    assert_eq!(
        (
            f.unlocked_liquidity,
            f.permanent_locked_liquidity,
            f.vested_liquidity,
            f.fee_a,
            f.reward_1
        ),
        (1, 2, 3, 4, 7)
    );
    let n = &split.numerators;
    assert_eq!(
        (
            n.unlocked_liquidity,
            n.permanent_locked_liquidity,
            n.fee_a,
            n.inner_vesting_liquidity
        ),
        (1, 2, 3, 7)
    );
}
