//! `PgPositionChangeEventRepository` under `yog_api`: the five kinds of position change merged
//! into one list from the start of a window, the most recent first, paged without repeat nor
//! gap through instants shared across tables.

use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::{AssertSqlSafe, PgPool};
use yog_core::{
    Cursor, Page,
    domain::{
        EventScope, MeteoraDammV2ClosePositionEvent, MeteoraDammV2CreatePositionEvent,
        MeteoraDammV2LockPositionEvent, MeteoraDammV2PermanentLockPositionEvent,
        MeteoraDammV2PositionChange, MeteoraDammV2SplitAmounts, MeteoraDammV2SplitNumerators,
        MeteoraDammV2SplitPositionEvent, MeteoraDammV2SplitPositionState, PositionChangeEvent,
        PositionChangeEventLookup, Protocol,
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
        // ⚠️ A cursor that rereads its own row would page forever: fail instead of hanging.
        assert!(seen.len() <= 100, "paging does not end: {seen:?}");
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
    let by_one = read_all(&repository, EventScope::All, 1).await;
    let by_two = read_all(&repository, EventScope::All, 2).await;
    let pool_by_two = read_all(&repository, EventScope::Pool(pk(1)), 2).await;

    assert_eq!(expected.len(), 7);
    assert_eq!(by_one, expected);
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

/// Mutation: a column of a table read into another field, or a first and second side of a
/// split swapped — a change that comes back other than written. Every column holds its own value.
#[sqlx::test(migrations = "./migrations")]
async fn each_kind_comes_back_as_written(pool: PgPool) {
    let base = |table: &str, columns: &str, values: &str| {
        format!(
            "INSERT INTO meteora_damm_v2_{table}_events
               (pool_address, signature, timestamp, slot, event_index, transaction_index, {columns})
             VALUES ($1, $2, $3, 9, 0, NULL, {values})"
        )
    };
    let insert = |sql: String, seed: u8, seconds: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query(AssertSqlSafe(sql))
                .bind(pk(1).to_string())
                .bind(sig(seed).to_string())
                .bind(at(seconds))
                .execute(&pool)
                .await
                .unwrap();
        }
    };
    let k = |seed: u8| format!("'{}'", pk(seed));
    insert(
        base(
            "create_position",
            "owner, position, position_nft_mint",
            &format!("{}, {}, {}", k(61), k(62), k(63)),
        ),
        1,
        10,
    )
    .await;
    insert(
        base(
            "close_position",
            "owner, position, position_nft_mint",
            &format!("{}, {}, {}", k(64), k(65), k(66)),
        ),
        2,
        9,
    )
    .await;
    insert(
        base(
            "lock_position",
            "position, owner, vesting, cliff_point, period_frequency, cliff_unlock_liquidity, \
         liquidity_per_period, number_of_period",
            &format!(
                "{}, {}, {}, 1, 2, 18446744073709551616, 4, 5",
                k(67),
                k(68),
                k(69)
            ),
        ),
        3,
        8,
    )
    .await;
    insert(
        base(
            "permanent_lock_position",
            "position, lock_liquidity_amount, total_permanent_locked_liquidity",
            &format!("{}, 6, 7", k(70)),
        ),
        4,
        7,
    )
    .await;
    insert(
        base(
            "split_position",
            "first_owner, second_owner, first_position, second_position, current_sqrt_price, \
         split_permanent_locked_liquidity, split_unlocked_liquidity, split_vested_liquidity, \
         split_fee_a, split_fee_b, split_reward_0, split_reward_1, \
         first_unlocked_liquidity, first_permanent_locked_liquidity, first_vested_liquidity, \
         first_fee_a, first_fee_b, first_reward_0, first_reward_1, \
         second_unlocked_liquidity, second_permanent_locked_liquidity, second_vested_liquidity, \
         second_fee_a, second_fee_b, second_reward_0, second_reward_1, \
         num_unlocked_liquidity, num_permanent_locked_liquidity, num_fee_a, num_fee_b, \
         num_reward_0, num_reward_1, num_inner_vesting_liquidity",
            &format!(
                "{}, {}, {}, {}, 8, 1, 2, 3, 4, 5, 6, 7, 11, 12, 13, 14, 15, 16, 17, \
             21, 22, 23, 24, 25, 26, 27, 31, 32, 33, 34, 35, 36, 37",
                k(71),
                k(72),
                k(73),
                k(74)
            ),
        ),
        5,
        6,
    )
    .await;
    let repository =
        PgPositionChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let page = repository
        .list_events(EventScope::All, at(-60), None, 50)
        .await
        .unwrap();

    macro_rules! event {
        ($ty:ident, $seed:expr, $seconds:expr, { $($field:ident: $value:expr),* $(,)? }) => {
            $ty {
                pool_address: pk(1),
                signature: sig($seed),
                timestamp: at($seconds),
                slot: 9,
                transaction_index: None,
                event_index: 0,
                $($field: $value),*
            }
        };
    }
    let state = |base: u64| MeteoraDammV2SplitPositionState {
        unlocked_liquidity: u128::from(base + 1),
        permanent_locked_liquidity: u128::from(base + 2),
        vested_liquidity: u128::from(base + 3),
        fee_a: base + 4,
        fee_b: base + 5,
        reward_0: base + 6,
        reward_1: base + 7,
    };
    let expected = [
        MeteoraDammV2PositionChange::Open(event!(MeteoraDammV2CreatePositionEvent, 1, 10, {
            owner: pk(61), position: pk(62), position_nft_mint: pk(63),
        })),
        MeteoraDammV2PositionChange::Close(event!(MeteoraDammV2ClosePositionEvent, 2, 9, {
            owner: pk(64), position: pk(65), position_nft_mint: pk(66),
        })),
        MeteoraDammV2PositionChange::VestingLock(event!(MeteoraDammV2LockPositionEvent, 3, 8, {
            position: pk(67), owner: pk(68), vesting: pk(69), cliff_point: 1,
            period_frequency: 2, cliff_unlock_liquidity: 1 << 64, liquidity_per_period: 4,
            number_of_period: 5,
        })),
        MeteoraDammV2PositionChange::PermanentLock(event!(
            MeteoraDammV2PermanentLockPositionEvent, 4, 7, {
                position: pk(70), lock_liquidity_amount: 6, total_permanent_locked_liquidity: 7,
            }
        )),
        MeteoraDammV2PositionChange::Split(Box::new(
            event!(MeteoraDammV2SplitPositionEvent, 5, 6, {
                first_owner: pk(71), second_owner: pk(72), first_position: pk(73),
                second_position: pk(74), current_sqrt_price: 8,
                amounts: MeteoraDammV2SplitAmounts {
                    permanent_locked_liquidity: 1, unlocked_liquidity: 2, vested_liquidity: 3,
                    fee_a: 4, fee_b: 5, reward_0: 6, reward_1: 7,
                },
                first_position_after: state(10),
                second_position_after: state(20),
                numerators: MeteoraDammV2SplitNumerators {
                    unlocked_liquidity: 31, permanent_locked_liquidity: 32, fee_a: 33, fee_b: 34,
                    reward_0: 35, reward_1: 36, inner_vesting_liquidity: 37,
                },
            }),
        )),
    ];

    let read: Vec<_> = page
        .items
        .iter()
        .map(|PositionChangeEvent::MeteoraDammV2(change)| format!("{change:?}"))
        .collect();
    let written: Vec<_> = expected
        .iter()
        .map(|change| format!("{change:?}"))
        .collect();
    assert_eq!(read, written);
}
