//! The projection orders on the event's position, not on a timestamp it shares with half
//! the pool's other swaps: a routed transaction's second hop must win over its first.

use chrono::{DateTime, Duration, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::PgPool;
use yog_core::domain::{
    EventPosition, LastEventKind, MeteoraDammV2LiquidityEventKind, PoolCurrentStateLookup,
    PoolCurrentStateRepository, PoolCurrentStateUpsert, Protocol,
};
use yog_persistence::PgPoolCurrentStateRepository;

use super::helpers::{pk, ts};

fn signature(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

/// ⚠️ Every event carries the same timestamp: the guard must order without it.
fn position(slot: u64, event_index: u16, signature_seed: u8) -> EventPosition {
    EventPosition {
        signature: signature(signature_seed),
        timestamp: ts(),
        slot,
        transaction_index: None,
        event_index,
    }
}

fn swap(event_position: EventPosition, sqrt_price: u128) -> PoolCurrentStateUpsert {
    PoolCurrentStateUpsert::from_swap(
        pk(1),
        Protocol::MeteoraDammV2,
        event_position,
        100,
        200,
        sqrt_price,
    )
}

/// The projection's foreign key needs the pool, which the indexer always discovers first.
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

/// Mutation: the guard back on `last_event_at <` — the second hop is rejected.
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

/// Mutation: a guard that accepts everything, which would also pass the test above.
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

/// Mutation: the guard reduced to `event_index` — the tuple compares slot first.
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

/// Mutation: `same_slot_ambiguity` always false. `transaction_index` is empty on this path,
/// so two transactions of one block cannot be ranked.
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

    let same_block = repo
        .upsert(&swap(position(300, 4, 2), 4_444))
        .await
        .unwrap();
    assert!(same_block.applied);
    assert!(
        same_block.same_slot_ambiguity,
        "same slot, different signature: this is the case the key cannot rank"
    );

    // Reported on the rejected path too.
    let rejected = repo
        .upsert(&swap(position(300, 1, 3), 1_000))
        .await
        .unwrap();
    assert!(!rejected.applied);
    assert!(
        rejected.same_slot_ambiguity,
        "counting only the applied path would understate the ambiguity"
    );

    let same_tx = repo
        .upsert(&swap(position(300, 9, 2), 9_999))
        .await
        .unwrap();
    assert!(same_tx.applied);
    assert!(!same_tx.same_slot_ambiguity);
}

/// `last_event_at` is still written — `/latest-state` displays it — though it decides nothing.
#[sqlx::test]
async fn last_event_at_is_still_recorded_though_it_no_longer_orders(pool: PgPool) {
    seed_pool(&pool).await;
    let repo = PgPoolCurrentStateRepository::new(pool.clone());
    repo.upsert(&swap(position(300, 0, 1), 1_111))
        .await
        .unwrap();

    assert_eq!(stored_event_at(&pool).await, ts());
}

/// ⚠️ `reserve_a` differs between the two writes: without a marker, "the later one landed"
/// is unobservable and a broken guard passes.
fn liquidity_upsert(event_position: EventPosition, reserve_a: u64) -> PoolCurrentStateUpsert {
    PoolCurrentStateUpsert::from_liquidity(
        pk(1),
        Protocol::MeteoraDammV2,
        event_position,
        MeteoraDammV2LiquidityEventKind::Add,
        reserve_a,
        200,
    )
}

/// Liquidity events share the guard, but build their payload elsewhere.
#[sqlx::test]
async fn the_liquidity_path_shares_the_same_ordering(pool: PgPool) {
    seed_pool(&pool).await;
    let repo = PgPoolCurrentStateRepository::new(pool.clone());

    assert!(
        repo.upsert(&liquidity_upsert(position(300, 0, 1), 10))
            .await
            .unwrap()
            .applied
    );

    let later = repo
        .upsert(&liquidity_upsert(position(300, 1, 1), 20))
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
        .reserve_a;
    assert_eq!(stored, 20, "the second write must be the one that stuck");
}

/// Mutation: either `COALESCE` replaced by `EXCLUDED.x`, or the `match` on the event kind
/// dropped. ⚠️ The liquidity event is a minute later, unlike the module's convention: with
/// the same timestamp, "preserved" and "overwritten identically" would look alike.
#[sqlx::test]
async fn a_liquidity_event_preserves_the_price_left_by_the_last_swap(pool: PgPool) {
    seed_pool(&pool).await;
    let repo = PgPoolCurrentStateRepository::new(pool.clone());

    assert!(
        repo.upsert(&swap(position(300, 0, 1), 1_111))
            .await
            .unwrap()
            .applied
    );

    let liquidity_at = ts() + Duration::seconds(60);
    let later_position = EventPosition {
        timestamp: liquidity_at,
        ..position(300, 1, 1)
    };
    assert!(
        repo.upsert(&liquidity_upsert(later_position, 42))
            .await
            .unwrap()
            .applied
    );

    let stored = repo
        .get_by_address(&pk(1).to_string())
        .await
        .unwrap()
        .expect("row");

    assert_eq!(
        stored.last_sqrt_price,
        Some(1_111),
        "the liquidity event carries no sqrt_price — the swap's must survive it"
    );
    assert_eq!(
        stored.last_swap_at,
        Some(ts()),
        "a liquidity event is not a swap: it must neither null this timestamp \
         nor re-stamp it with its own ({liquidity_at})"
    );
    // The liquidity event's own columns landed: the write was not merely rejected.
    assert_eq!(stored.reserve_a, 42);
    assert_eq!(stored.last_event_kind, LastEventKind::LiquidityAdd);
    assert_eq!(
        stored.last_event_at, liquidity_at,
        "the projection's own clock does follow the latest event"
    );
}

/// The mainnet transaction `2qJrr…` through the real extractor must leave its result, not
/// its first hop. Mutation: the guard back on `last_event_at`.
#[sqlx::test]
async fn a_routed_transaction_leaves_its_result_not_its_first_hop(pool: PgPool) {
    let swaps = super::event_index_uniqueness::swap_double_swaps();
    assert_eq!(swaps.len(), 2, "the fixture must carry exactly two swaps");
    assert_eq!(swaps[0].signature, swaps[1].signature);
    assert_eq!(swaps[0].timestamp, swaps[1].timestamp);
    assert!(
        swaps[0].event_index < swaps[1].event_index,
        "the fixture's legs must be ordered by event_index for this to mean anything"
    );

    let pool_address = swaps[0].pool_address;
    sqlx::query("INSERT INTO pools (pool_address, protocol) VALUES ($1, 'meteora_damm_v2')")
        .bind(pool_address.to_string())
        .execute(&pool)
        .await
        .unwrap();

    let repo = PgPoolCurrentStateRepository::new(pool.clone());
    for swap in &swaps {
        let outcome = repo
            .upsert(&PoolCurrentStateUpsert::from_swap(
                swap.pool_address,
                Protocol::MeteoraDammV2,
                EventPosition {
                    signature: swap.signature,
                    timestamp: swap.timestamp,
                    slot: swap.slot,
                    transaction_index: swap.transaction_index,
                    event_index: swap.event_index,
                },
                swap.reserve_a_after,
                swap.reserve_b_after,
                swap.next_sqrt_price,
            ))
            .await
            .unwrap();
        assert!(
            outcome.applied,
            "leg {} was rejected — the projection is dropping part of the transaction",
            swap.event_index
        );
        assert!(
            !outcome.same_slot_ambiguity,
            "one signature, one order: two legs of the same transaction are not ambiguous"
        );
    }

    let state = repo
        .get_by_address(&pool_address.to_string())
        .await
        .unwrap()
        .expect("the projection row must exist");
    let last = &swaps[1];
    assert_eq!(state.last_sqrt_price, Some(last.next_sqrt_price));
    assert_eq!(state.reserve_a, last.reserve_a_after);
    assert_eq!(state.reserve_b, last.reserve_b_after);
}
