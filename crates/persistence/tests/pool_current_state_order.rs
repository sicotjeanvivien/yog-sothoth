//! The projection must order on the event's position, not on a timestamp it
//! shares with half the pool's other swaps.
//!
//! Regression guard for the finding measured on 3 August 2026: the upsert was
//! guarded by `last_event_at <` — a `blockTime`, so a **second** — while
//! 56,1 % of swaps share their `(pool, timestamp)` with another swap, up to 46
//! within one second. **33,5 % of state updates were rejected** and labelled
//! `stale`, as if they were healthy concurrency.
//!
//! Its most visible consequence: both legs of a routed transaction persist,
//! but the *first* wins the projection, so the pool shows intermediate
//! reserves and an intermediate `sqrt_price` — never the transaction's result.

use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::PgPool;
use yog_core::domain::{
    EventPosition, MeteoraDammV2LiquidityEventKind, PoolCurrentStateLookup,
    PoolCurrentStateRepository, PoolCurrentStateUpsert, Protocol,
};
use yog_persistence::PgPoolCurrentStateRepository;

use super::helpers::{pk, ts};

fn signature(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

/// Every event of these tests carries the SAME timestamp on purpose: that is
/// the situation the old guard could not handle, and the one the new key has
/// to resolve without it.
fn position(slot: u64, event_index: u16, signature_seed: u8) -> EventPosition {
    EventPosition {
        signature: signature(signature_seed),
        timestamp: ts(),
        slot,
        transaction_index: None,
        event_index,
    }
}

fn swap(position: EventPosition, sqrt_price: u128) -> PoolCurrentStateUpsert {
    PoolCurrentStateUpsert::from_swap(
        pk(1),
        Protocol::MeteoraDammV2,
        position,
        100,
        200,
        sqrt_price,
    )
}

/// `pool_current_state` carries a foreign key to `pools`: the projection can
/// only describe a pool the registry has seen. The indexer always discovers
/// the pool before projecting onto it (`discover_pool` runs first in the
/// persistor), so seeding it here reproduces the real order.
async fn seed_pool(pool: &PgPool) {
    sqlx::query("INSERT INTO pools (pool_address, protocol) VALUES ($1, 'meteora_damm_v2')")
        .bind(pk(1).to_string())
        .execute(pool)
        .await
        .unwrap();
}

async fn stored_sqrt_price(repo: &PgPoolCurrentStateRepository, pool: Pubkey) -> Option<u128> {
    repo.get_by_address(&pool.to_string())
        .await
        .unwrap()
        .expect("the projection row must exist")
        .last_sqrt_price
}

async fn stored_event_at(pool: &PgPool) -> DateTime<Utc> {
    sqlx::query_scalar("SELECT last_event_at FROM pool_current_state")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The second hop of a transaction wins, though it shares its second with the
/// first. This is the defect, stated positively.
///
/// Mutation-checked: put the guard back to
/// `pool_current_state.last_event_at < EXCLUDED.last_event_at` and this fails —
/// the second upsert is rejected and the pool keeps the first hop's price.
#[sqlx::test]
async fn a_later_event_of_the_same_second_wins(pool: PgPool) {
    seed_pool(&pool).await;
    let repo = PgPoolCurrentStateRepository::new(pool.clone());

    let first = repo
        .upsert(&swap(position(300, 0, 1), 1_111))
        .await
        .unwrap();
    assert!(first.applied);

    let second = repo
        .upsert(&swap(position(300, 1, 1), 2_222))
        .await
        .unwrap();
    assert!(
        second.applied,
        "same signature, same second, higher event_index: this is the second \
         hop of one transaction and it must win"
    );
    assert_eq!(stored_sqrt_price(&repo, pk(1)).await, Some(2_222));
}

/// …and the symmetric case: an event that is *earlier* in the same transaction
/// must still be rejected. A guard that accepted everything would also make
/// the first test pass.
#[sqlx::test]
async fn an_earlier_event_of_the_same_second_is_rejected(pool: PgPool) {
    seed_pool(&pool).await;
    let repo = PgPoolCurrentStateRepository::new(pool.clone());

    assert!(
        repo.upsert(&swap(position(300, 5, 1), 5_555))
            .await
            .unwrap()
            .applied
    );

    let out_of_order = repo
        .upsert(&swap(position(300, 2, 1), 2_222))
        .await
        .unwrap();
    assert!(
        !out_of_order.applied,
        "a lower event_index of the same transaction is older — it must not \
         overwrite the state"
    );
    assert_eq!(
        stored_sqrt_price(&repo, pk(1)).await,
        Some(5_555),
        "the rejected upsert must have left the stored state untouched"
    );
}

/// Slot beats event_index: the tuple is compared left to right, so an event
/// from a later block wins even with a smaller index.
///
/// Mutation-checked: reduce the guard to `event_index` alone and this fails.
#[sqlx::test]
async fn a_later_slot_wins_over_a_higher_event_index(pool: PgPool) {
    seed_pool(&pool).await;
    let repo = PgPoolCurrentStateRepository::new(pool.clone());

    assert!(
        repo.upsert(&swap(position(300, 9, 1), 9_999))
            .await
            .unwrap()
            .applied
    );

    let next_block = repo
        .upsert(&swap(position(301, 0, 2), 1_000))
        .await
        .unwrap();
    assert!(
        next_block.applied,
        "slot 301 comes after slot 300 whatever the event_index says"
    );
    assert_eq!(stored_sqrt_price(&repo, pk(1)).await, Some(1_000));
}

/// Two transactions in one block, on one pool: `transaction_index` is empty on
/// this ingestion path, so `(slot, _, event_index)` cannot say which came
/// first. The repository does not pretend otherwise — it reports the
/// ambiguity so the indexer can count it.
///
/// Mutation-checked: drop the `previous` CTE's contribution (always report
/// `same_slot_ambiguity: false`) and this fails.
#[sqlx::test]
async fn same_slot_different_signature_is_reported_as_ambiguous(pool: PgPool) {
    seed_pool(&pool).await;
    let repo = PgPoolCurrentStateRepository::new(pool.clone());

    let first = repo
        .upsert(&swap(position(300, 0, 1), 1_111))
        .await
        .unwrap();
    assert!(
        !first.same_slot_ambiguity,
        "the very first upsert meets no previous state at all"
    );

    // Another transaction of the same block. Applied here — but the point is
    // that the guard cannot know it deserved to be.
    let same_block = repo
        .upsert(&swap(position(300, 4, 2), 4_444))
        .await
        .unwrap();
    assert!(same_block.applied);
    assert!(
        same_block.same_slot_ambiguity,
        "same slot, different signature: this is the case the key cannot rank"
    );

    // Reported on the rejected path too — an ambiguity that wrongly accepts
    // costs as much as one that wrongly rejects.
    let rejected = repo
        .upsert(&swap(position(300, 1, 3), 1_000))
        .await
        .unwrap();
    assert!(!rejected.applied);
    assert!(
        rejected.same_slot_ambiguity,
        "counting only the applied path would understate the ambiguity"
    );

    // A second hop of the *same* transaction is not ambiguous: one signature,
    // one order, given by event_index.
    let same_tx = repo
        .upsert(&swap(position(300, 9, 2), 9_999))
        .await
        .unwrap();
    assert!(same_tx.applied);
    assert!(!same_tx.same_slot_ambiguity);
}

/// `last_event_at` keeps being written — it is what `/latest-state` displays —
/// it simply stopped deciding anything.
#[sqlx::test]
async fn last_event_at_is_still_recorded_though_it_no_longer_orders(pool: PgPool) {
    seed_pool(&pool).await;
    let repo = PgPoolCurrentStateRepository::new(pool.clone());
    repo.upsert(&swap(position(300, 0, 1), 1_111))
        .await
        .unwrap();

    assert_eq!(stored_event_at(&pool).await, ts());
}

/// Liquidity events share the upsert, so they share the guard. Worth its own
/// test rather than an assumption: the two paths build their payload in
/// different places.
#[sqlx::test]
async fn the_liquidity_path_shares_the_same_ordering(pool: PgPool) {
    seed_pool(&pool).await;
    let repo = PgPoolCurrentStateRepository::new(pool.clone());

    let liquidity = |position: EventPosition, delta: u128| {
        PoolCurrentStateUpsert::from_liquidity(
            pk(1),
            Protocol::MeteoraDammV2,
            position,
            MeteoraDammV2LiquidityEventKind::Add,
            100,
            200,
            delta,
        )
    };

    assert!(
        repo.upsert(&liquidity(position(300, 0, 1), 10))
            .await
            .unwrap()
            .applied
    );

    let later = repo
        .upsert(&liquidity(position(300, 1, 1), 20))
        .await
        .unwrap();
    assert!(
        later.applied,
        "same second, higher event_index — the liquidity path must order like \
         the swap path"
    );

    let stored = repo
        .get_by_address(&pk(1).to_string())
        .await
        .unwrap()
        .expect("row")
        .liquidity;
    assert_eq!(stored, Some(20));
}
