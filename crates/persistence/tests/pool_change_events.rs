//! `PgPoolChangeEventRepository` under `yog_api`: the two kinds of pool change merged into one
//! list from the start of a window, the most recent first, paged without repeat nor gap through
//! instants shared across tables.

use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::{AssertSqlSafe, PgPool};
use yog_core::{
    Cursor, Page,
    domain::{
        EventScope, MeteoraDammV2PoolChange, MeteoraDammV2SetPoolStatusEvent,
        MeteoraDammV2UpdatePoolFeesEvent, PoolChangeEvent, PoolChangeEventLookup, Protocol,
    },
};
use yog_persistence::PgPoolChangeEventRepository;

use super::helpers::{pk, setup_roles_and_connect_as};

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::seconds(seconds)
}

fn sig(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Status,
    Fees,
}

const KINDS: [Kind; 2] = [Kind::Status, Kind::Fees];

async fn insert_change(
    pool: &PgPool,
    kind: Kind,
    address: Pubkey,
    signature: Signature,
    index: i32,
    seconds: i64,
) {
    let sql = match kind {
        Kind::Status => {
            "INSERT INTO meteora_damm_v2_set_pool_status_events
               (pool_address, signature, timestamp, slot, event_index, transaction_index, status)
             VALUES ($1, $2, $3, 9, $4, NULL, 0)"
        }
        Kind::Fees => {
            "INSERT INTO meteora_damm_v2_update_pool_fees_events
               (pool_address, signature, timestamp, slot, event_index, transaction_index, operator, params_raw)
             VALUES ($1, $2, $3, 9, $4, NULL, $5, '\\x01')"
        }
    };
    let query = sqlx::query(sql)
        .bind(address.to_string())
        .bind(signature.to_string())
        .bind(at(seconds))
        .bind(index);
    let query = if sql.contains("$5") {
        query.bind(pk(50).to_string())
    } else {
        query
    };
    query.execute(pool).await.unwrap();
}

fn kind_of(change: &MeteoraDammV2PoolChange) -> Kind {
    match change {
        MeteoraDammV2PoolChange::Status(_) => Kind::Status,
        MeteoraDammV2PoolChange::Fees(_) => Kind::Fees,
    }
}

type Key = (DateTime<Utc>, String, u16, Kind);

fn keys(page: &Page<PoolChangeEvent>) -> Vec<Key> {
    page.items
        .iter()
        .map(|PoolChangeEvent::MeteoraDammV2(change)| {
            let (at, signature, index) = change.key();
            (at, signature.to_string(), index, kind_of(change))
        })
        .collect()
}

async fn read_all(
    repository: &PgPoolChangeEventRepository,
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
            Some(Cursor::PoolChangeEvent(next)) => cursor = Some(*next),
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
    // Every kind at one instant, two of them one transaction: page boundaries fall among them.
    for (kind, seed, index, seconds) in [
        (Kind::Status, 3, 0, 10),
        (Kind::Fees, 3, 1, 10),
        (Kind::Status, 11, 0, 9),
        (Kind::Fees, 12, 0, 8),
    ] {
        insert_change(&pool, kind, pk(1), sig(seed), index, seconds).await;
    }
    let repository =
        PgPoolChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let expected = read_all(&repository, EventScope::All, 50).await;
    let by_one = read_all(&repository, EventScope::All, 1).await;
    let by_two = read_all(&repository, EventScope::All, 2).await;
    let pool_by_two = read_all(&repository, EventScope::Pool(pk(1)), 2).await;

    assert_eq!(expected.len(), 4);
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
    assert_eq!(one_transaction, [(1, Kind::Fees), (0, Kind::Status)]);
    for kind in KINDS {
        assert!(expected.iter().any(|k| k.3 == kind), "{kind:?}");
    }
}

/// Mutation: the pool filter of one table, the protocol dispatch or the window's start dropped
/// — another pool's change, a DLMM page, or a change before the window, listed.
#[sqlx::test(migrations = "./migrations")]
async fn a_scope_and_its_window_read_only_their_changes(pool: PgPool) {
    for kind in KINDS {
        let seed = kind as u8;
        insert_change(&pool, kind, pk(1), sig(10 + seed), 0, 10).await;
        insert_change(&pool, kind, pk(2), sig(20 + seed), 0, 20).await;
        insert_change(&pool, kind, pk(1), sig(30 + seed), 0, -120).await;
    }
    let repository =
        PgPoolChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let mine = read_all(&repository, EventScope::Pool(pk(1)), 50).await;
    let all = read_all(&repository, EventScope::All, 50).await;
    let damm = read_all(
        &repository,
        EventScope::Protocol(Protocol::MeteoraDammV2),
        50,
    )
    .await;
    let dlmm = read_all(&repository, EventScope::Protocol(Protocol::MeteoraDlmm), 50).await;

    assert_eq!(mine.len(), KINDS.len());
    assert!(mine.iter().all(|k| k.0 == at(10)), "{mine:?}");
    assert_eq!(all.len(), 2 * KINDS.len());
    assert_eq!(damm, all);
    assert!(dlmm.is_empty());
}

/// Mutation: a column of a table read into another field — a change that comes back other than
/// written. Every column holds its own value.
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
    insert(base("set_pool_status", "status", "7"), 1, 10).await;
    insert(
        base(
            "update_pool_fees",
            "operator, params_raw",
            &format!("{k61}, '\\x0102'", k61 = k(61)),
        ),
        2,
        9,
    )
    .await;
    let repository =
        PgPoolChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

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
    let expected = [
        MeteoraDammV2PoolChange::Status(
            event!(MeteoraDammV2SetPoolStatusEvent, 1, 10, { status: 7, }),
        ),
        MeteoraDammV2PoolChange::Fees(
            event!(MeteoraDammV2UpdatePoolFeesEvent, 2, 9, { operator: pk(61), params_raw: vec![1, 2], }),
        ),
    ];

    let read: Vec<_> = page
        .items
        .iter()
        .map(|PoolChangeEvent::MeteoraDammV2(change)| format!("{change:?}"))
        .collect();
    let written: Vec<_> = expected
        .iter()
        .map(|change| format!("{change:?}"))
        .collect();
    assert_eq!(read, written);
}
