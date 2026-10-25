//! `PgRewardChangeEventRepository` under `yog_api`: the six kinds of reward change merged into
//! one list from the start of a window, the most recent first, paged without repeat nor gap
//! through instants shared across tables.

use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::{AssertSqlSafe, PgPool};
use yog_core::{
    Cursor, Page,
    domain::{
        EventScope, MeteoraDammV2FundRewardEvent, MeteoraDammV2InitializeRewardEvent,
        MeteoraDammV2RewardChange, MeteoraDammV2UpdateRewardDurationEvent,
        MeteoraDammV2UpdateRewardFunderEvent, MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
        MeteoraDammV2WithdrawIneligibleRewardEvent, Protocol, RewardChangeEvent,
        RewardChangeEventLookup,
    },
};
use yog_persistence::PgRewardChangeEventRepository;

use super::helpers::{pk, setup_roles_and_connect_as};

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::seconds(seconds)
}

fn sig(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Initialize,
    Fund,
    DurationChange,
    FunderChange,
    IneligibleWithdrawal,
    DeadLiquidityWithdrawal,
}

const KINDS: [Kind; 6] = [
    Kind::Initialize,
    Kind::Fund,
    Kind::DurationChange,
    Kind::FunderChange,
    Kind::IneligibleWithdrawal,
    Kind::DeadLiquidityWithdrawal,
];

async fn insert_change(
    pool: &PgPool,
    kind: Kind,
    address: Pubkey,
    signature: Signature,
    index: i32,
    seconds: i64,
) {
    let sql = match kind {
        Kind::Initialize => {
            "INSERT INTO meteora_damm_v2_initialize_reward_events
               (pool_address, signature, timestamp, slot, event_index, transaction_index, reward_mint, funder, creator, reward_index, reward_duration)
             VALUES ($1, $2, $3, 9, $4, NULL, $5, $5, $5, 0, 60)"
        }
        Kind::Fund => {
            "INSERT INTO meteora_damm_v2_fund_reward_events
               (pool_address, signature, timestamp, slot, event_index, transaction_index, funder, mint_reward, reward_index, amount, transfer_fee_excluded_amount_in, reward_duration_end, pre_reward_rate, post_reward_rate)
             VALUES ($1, $2, $3, 9, $4, NULL, $5, $5, 0, 1, 1, 2, 3, 4)"
        }
        Kind::DurationChange => {
            "INSERT INTO meteora_damm_v2_update_reward_duration_events
               (pool_address, signature, timestamp, slot, event_index, transaction_index, reward_index, old_reward_duration, new_reward_duration)
             VALUES ($1, $2, $3, 9, $4, NULL, 0, 1, 2)"
        }
        Kind::FunderChange => {
            "INSERT INTO meteora_damm_v2_update_reward_funder_events
               (pool_address, signature, timestamp, slot, event_index, transaction_index, reward_index, old_funder, new_funder)
             VALUES ($1, $2, $3, 9, $4, NULL, 0, $5, $5)"
        }
        Kind::IneligibleWithdrawal => {
            "INSERT INTO meteora_damm_v2_withdraw_ineligible_reward_events
               (pool_address, signature, timestamp, slot, event_index, transaction_index, reward_mint, amount)
             VALUES ($1, $2, $3, 9, $4, NULL, $5, 0)"
        }
        Kind::DeadLiquidityWithdrawal => {
            "INSERT INTO meteora_damm_v2_withdraw_dead_liquidity_reward_events
               (pool_address, signature, timestamp, slot, event_index, transaction_index, reward_mint, amount)
             VALUES ($1, $2, $3, 9, $4, NULL, $5, 1)"
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

fn kind_of(change: &MeteoraDammV2RewardChange) -> Kind {
    match change {
        MeteoraDammV2RewardChange::Initialize(_) => Kind::Initialize,
        MeteoraDammV2RewardChange::Fund(_) => Kind::Fund,
        MeteoraDammV2RewardChange::DurationChange(_) => Kind::DurationChange,
        MeteoraDammV2RewardChange::FunderChange(_) => Kind::FunderChange,
        MeteoraDammV2RewardChange::IneligibleWithdrawal(_) => Kind::IneligibleWithdrawal,
        MeteoraDammV2RewardChange::DeadLiquidityWithdrawal(_) => Kind::DeadLiquidityWithdrawal,
    }
}

type Key = (DateTime<Utc>, String, u16, Kind);

fn keys(page: &Page<RewardChangeEvent>) -> Vec<Key> {
    page.items
        .iter()
        .map(|RewardChangeEvent::MeteoraDammV2(change)| {
            let (at, signature, index) = change.key();
            (at, signature.to_string(), index, kind_of(change))
        })
        .collect()
}

async fn read_all(
    repository: &PgRewardChangeEventRepository,
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
            Some(Cursor::RewardChangeEvent(next)) => cursor = Some(*next),
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
        (Kind::Initialize, 3, 0, 10),
        (Kind::Fund, 3, 1, 10),
        (Kind::DurationChange, 1, 0, 10),
        (Kind::FunderChange, 2, 0, 10),
        (Kind::IneligibleWithdrawal, 4, 0, 10),
        (Kind::DeadLiquidityWithdrawal, 6, 0, 10),
        (Kind::Initialize, 11, 0, 9),
        (Kind::DeadLiquidityWithdrawal, 12, 0, 8),
    ] {
        insert_change(&pool, kind, pk(1), sig(seed), index, seconds).await;
    }
    let repository =
        PgRewardChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let expected = read_all(&repository, EventScope::All, 50).await;
    let by_one = read_all(&repository, EventScope::All, 1).await;
    let by_two = read_all(&repository, EventScope::All, 2).await;
    let pool_by_two = read_all(&repository, EventScope::Pool(pk(1)), 2).await;

    assert_eq!(expected.len(), 8);
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
    assert_eq!(one_transaction, [(1, Kind::Fund), (0, Kind::Initialize)]);
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
        PgRewardChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

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
/// written.
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
            "initialize_reward",
            "reward_mint, funder, creator, reward_index, reward_duration",
            &format!(
                "{k61}, {k62}, {k63}, 1, 3600",
                k61 = k(61),
                k62 = k(62),
                k63 = k(63)
            ),
        ),
        1,
        10,
    )
    .await;
    insert(
        base("fund_reward", "funder, mint_reward, reward_index, amount, transfer_fee_excluded_amount_in, reward_duration_end, pre_reward_rate, post_reward_rate", &format!("{k64}, {k65}, 1, 11, 12, 13, 14, 18446744073709551616", k64 = k(64), k65 = k(65))),
        2,
        9,
    )
    .await;
    insert(
        base(
            "update_reward_duration",
            "reward_index, old_reward_duration, new_reward_duration",
            "1, 21, 22",
        ),
        3,
        8,
    )
    .await;
    insert(
        base(
            "update_reward_funder",
            "reward_index, old_funder, new_funder",
            &format!("1, {k66}, {k67}", k66 = k(66), k67 = k(67)),
        ),
        4,
        7,
    )
    .await;
    insert(
        base(
            "withdraw_ineligible_reward",
            "reward_mint, amount",
            &format!("{k68}, 31", k68 = k(68)),
        ),
        5,
        6,
    )
    .await;
    insert(
        base(
            "withdraw_dead_liquidity_reward",
            "reward_mint, amount",
            &format!("{k69}, 41", k69 = k(69)),
        ),
        6,
        5,
    )
    .await;
    let repository =
        PgRewardChangeEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

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
        MeteoraDammV2RewardChange::Initialize(
            event!(MeteoraDammV2InitializeRewardEvent, 1, 10, { reward_mint: pk(61), funder: pk(62), creator: pk(63), reward_index: 1, reward_duration: 3600, }),
        ),
        MeteoraDammV2RewardChange::Fund(
            event!(MeteoraDammV2FundRewardEvent, 2, 9, { funder: pk(64), mint_reward: pk(65), reward_index: 1, amount: 11, transfer_fee_excluded_amount_in: 12, reward_duration_end: 13, pre_reward_rate: 14, post_reward_rate: 1 << 64, }),
        ),
        MeteoraDammV2RewardChange::DurationChange(
            event!(MeteoraDammV2UpdateRewardDurationEvent, 3, 8, { reward_index: 1, old_reward_duration: 21, new_reward_duration: 22, }),
        ),
        MeteoraDammV2RewardChange::FunderChange(
            event!(MeteoraDammV2UpdateRewardFunderEvent, 4, 7, { reward_index: 1, old_funder: pk(66), new_funder: pk(67), }),
        ),
        MeteoraDammV2RewardChange::IneligibleWithdrawal(
            event!(MeteoraDammV2WithdrawIneligibleRewardEvent, 5, 6, { reward_mint: pk(68), amount: 31, }),
        ),
        MeteoraDammV2RewardChange::DeadLiquidityWithdrawal(
            event!(MeteoraDammV2WithdrawDeadLiquidityRewardEvent, 6, 5, { reward_mint: pk(69), amount: 41, }),
        ),
    ];

    let read: Vec<_> = page
        .items
        .iter()
        .map(|RewardChangeEvent::MeteoraDammV2(change)| format!("{change:?}"))
        .collect();
    let written: Vec<_> = expected
        .iter()
        .map(|change| format!("{change:?}"))
        .collect();
    assert_eq!(read, written);
}
