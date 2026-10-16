//! The unique key must tell apart two events of one transaction: a routed transaction emits
//! one per hop under one signature and one `blockTime`. ⚠️ The real extractor over a real
//! transaction: a synthetic pair would only prove that two rows made different stay different.

use sqlx::PgPool;
use yog_core::application::extraction::conformance::reference_transaction;
use yog_core::application::extraction::{EventExtractor, MeteoraDammV2};
use yog_core::domain::{
    DomainEvent, InsertOutcome, MeteoraDammV2Event, MeteoraDammV2SwapEvent,
    MeteoraDammV2SwapEventRepository,
};
use yog_persistence::PgMeteoraDammV2SwapEventRepository;

/// The mainnet transaction `2qJrr…`: two swaps on the same pool, opposite directions, one
/// signature, one `blockTime` — from `yog-core`'s conformance module, pinned to the mainnet response.
pub(super) fn swap_double_swaps() -> Vec<MeteoraDammV2SwapEvent> {
    let outcome = MeteoraDammV2::new()
        .extract_events(&reference_transaction())
        .expect("extraction failed on the reference transaction");

    outcome
        .events
        .into_iter()
        .filter_map(|e| match e {
            DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Swap(s)) => Some(s),
            _ => None,
        })
        .collect()
}

/// Mutation: the unique index and the `ON CONFLICT` target back to `(signature, timestamp)`.
#[sqlx::test]
async fn both_legs_of_a_routed_transaction_persist(pool: PgPool) {
    let swaps = swap_double_swaps();
    assert_eq!(swaps.len(), 2, "fixture must carry exactly two swaps");

    // Equal on everything the old key read, pool included.
    assert_eq!(swaps[0].signature, swaps[1].signature);
    assert_eq!(swaps[0].timestamp, swaps[1].timestamp);
    assert_eq!(swaps[0].pool_address, swaps[1].pool_address);
    assert_ne!(
        swaps[0].event_index, swaps[1].event_index,
        "the two legs must differ by event_index — nothing else separates them"
    );

    let repo = PgMeteoraDammV2SwapEventRepository::new(pool.clone());
    for swap in &swaps {
        assert_eq!(
            repo.insert(swap).await.unwrap(),
            InsertOutcome::Inserted,
            "leg {} was not written",
            swap.event_index
        );
    }

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_swap_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        rows, 2,
        "both legs of the routed transaction must be stored"
    );
}

/// A key wide enough to separate two events must still collapse a genuine replay, and say so.
#[sqlx::test]
async fn re_ingesting_the_same_event_is_reported_as_skipped(pool: PgPool) {
    let swap = swap_double_swaps().remove(0);
    let repo = PgMeteoraDammV2SwapEventRepository::new(pool.clone());

    assert_eq!(repo.insert(&swap).await.unwrap(), InsertOutcome::Inserted);
    assert_eq!(
        repo.insert(&swap).await.unwrap(),
        InsertOutcome::Skipped,
        "a replayed event must report the conflict, not a silent success"
    );

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_swap_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1, "the replay must not have duplicated the row");
}

/// Mutation: a `DEFAULT 0` left in place, or a bind in the wrong slot — zeros read back as
/// a plausible first event.
#[sqlx::test]
async fn position_columns_are_stored_as_extracted(pool: PgPool) {
    let swap = swap_double_swaps().remove(1);
    let repo = PgMeteoraDammV2SwapEventRepository::new(pool.clone());
    assert_eq!(repo.insert(&swap).await.unwrap(), InsertOutcome::Inserted);

    let (slot, event_index, transaction_index): (i64, i32, Option<i64>) = sqlx::query_as(
        "SELECT slot, event_index, transaction_index FROM meteora_damm_v2_swap_events",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(slot as u64, swap.slot);
    assert_eq!(event_index as u16, swap.event_index);
    assert_ne!(
        slot, 0,
        "slot must come from the transaction, not the default"
    );
    // Only that `None` survives: the value is a fixture constant, and no test watches a live
    // provider start returning `transactionIndex`.
    assert_eq!(transaction_index, None);
}

/// Mutation: the `event_index` clause removed from the forward predicate — a page boundary
/// between the two hops drops the second.
#[sqlx::test]
async fn paging_across_two_hops_of_one_transaction_skips_nothing(pool: PgPool) {
    use yog_core::domain::MeteoraDammV2SwapEventFeed;
    use yog_core::{Cursor, PageDirection};

    let swaps = swap_double_swaps();
    let pool_address = swaps[0].pool_address;
    let repo = PgMeteoraDammV2SwapEventRepository::new(pool.clone());
    for swap in &swaps {
        assert_eq!(repo.insert(swap).await.unwrap(), InsertOutcome::Inserted);
    }

    let first = repo
        .find_by_pool_paginated(&pool_address, None, PageDirection::Next, None, 1)
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);

    let Some(Cursor::MeteoraDammV2SwapEvent(cursor)) = first.next_cursor.clone() else {
        panic!(
            "expected a swap cursor to continue from, got {:?}",
            first.next_cursor
        );
    };

    let second = repo
        .find_by_pool_paginated(&pool_address, Some(cursor), PageDirection::Next, None, 1)
        .await
        .unwrap();
    assert_eq!(
        second.items.len(),
        1,
        "the second hop must be reachable — a two-key cursor loses it"
    );

    let mut seen = [first.items[0].event_index, second.items[0].event_index];
    seen.sort_unstable();
    assert_eq!(
        seen,
        [
            swaps[0].event_index.min(swaps[1].event_index),
            swaps[0].event_index.max(swaps[1].event_index)
        ],
        "the two pages must together cover both hops, each exactly once"
    );
}

/// Mutation: the `event_index` clause removed from the backward predicate.
#[sqlx::test]
async fn paging_backward_across_two_hops_skips_nothing(pool: PgPool) {
    use yog_core::domain::MeteoraDammV2SwapEventFeed;
    use yog_core::{Cursor, PageDirection, PagePosition};

    let swaps = swap_double_swaps();
    let pool_address = swaps[0].pool_address;
    let repo = PgMeteoraDammV2SwapEventRepository::new(pool.clone());
    for swap in &swaps {
        assert_eq!(repo.insert(swap).await.unwrap(), InsertOutcome::Inserted);
    }

    let last = repo
        .find_by_pool_paginated(
            &pool_address,
            None,
            PageDirection::Next,
            Some(PagePosition::Last),
            1,
        )
        .await
        .unwrap();
    assert_eq!(last.items.len(), 1);

    let Some(Cursor::MeteoraDammV2SwapEvent(cursor)) = last.prev_cursor.clone() else {
        panic!(
            "expected a cursor to walk back from, got {:?}",
            last.prev_cursor
        );
    };

    let previous = repo
        .find_by_pool_paginated(&pool_address, Some(cursor), PageDirection::Prev, None, 1)
        .await
        .unwrap();
    assert_eq!(
        previous.items.len(),
        1,
        "walking back must reach the other hop — a two-key cursor loses it"
    );
    assert_ne!(
        previous.items[0].event_index, last.items[0].event_index,
        "the two pages must be the two different hops, not the same row twice"
    );
}

/// The liquidity feed's own key and cursor. Built by hand: no fixture carries two liquidity
/// events of one transaction, and storage code needs no real data to be exercised.
#[sqlx::test]
async fn liquidity_feed_stores_and_pages_two_hops_of_one_transaction(pool: PgPool) {
    use yog_core::domain::{
        MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventFeed,
        MeteoraDammV2LiquidityEventKind, MeteoraDammV2LiquidityEventRepository,
    };
    use yog_core::{Cursor, PageDirection};
    use yog_persistence::PgMeteoraDammV2LiquidityEventRepository;

    let reference = swap_double_swaps().remove(0);
    let event =
        |event_index: u16, kind: MeteoraDammV2LiquidityEventKind| MeteoraDammV2LiquidityEvent {
            pool_address: reference.pool_address,
            signature: reference.signature,
            timestamp: reference.timestamp,
            slot: reference.slot,
            transaction_index: None,
            event_index,
            liquidity_event_kind: kind,
            amount_a: 1_000,
            amount_b: 2_000,
            liquidity_delta: 5_000,
            reserve_a_after: 10_000,
            reserve_b_after: 20_000,
            position: reference.pool_address,
            owner: reference.pool_address,
        };

    let repo = PgMeteoraDammV2LiquidityEventRepository::new(pool.clone());
    assert_eq!(
        repo.insert(&event(0, MeteoraDammV2LiquidityEventKind::Add))
            .await
            .unwrap(),
        InsertOutcome::Inserted
    );
    assert_eq!(
        repo.insert(&event(1, MeteoraDammV2LiquidityEventKind::Remove))
            .await
            .unwrap(),
        InsertOutcome::Inserted,
        "the second hop must be written — under the old key it conflicted"
    );

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_liquidity_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 2);

    // …and both are reachable across a page boundary, through the valued VIEW.
    let first = repo
        .find_by_pool_paginated(&reference.pool_address, None, PageDirection::Next, None, 1)
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);

    let Some(Cursor::MeteoraDammV2LiquidityEvent(cursor)) = first.next_cursor.clone() else {
        panic!("expected a liquidity cursor, got {:?}", first.next_cursor);
    };

    let second = repo
        .find_by_pool_paginated(
            &reference.pool_address,
            Some(cursor),
            PageDirection::Next,
            None,
            1,
        )
        .await
        .unwrap();
    assert_eq!(
        second.items.len(),
        1,
        "the second hop must be reachable — a two-key cursor loses it"
    );
    assert_ne!(
        first.items[0].event.event_index, second.items[0].event.event_index,
        "the two pages must be the two different hops"
    );
}
