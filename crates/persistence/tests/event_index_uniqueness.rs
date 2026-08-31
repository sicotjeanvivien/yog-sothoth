//! The unique key must tell apart two events emitted by one transaction.
//!
//! This is the regression guard for the defect measured on 4 August 2026: the
//! key was `(signature, timestamp)`, a routed transaction emits one event per
//! hop under a single signature and a single `blockTime`, and
//! `ON CONFLICT DO NOTHING` dropped every hop but one — 29 losses out of 482
//! emissions across three pools, silently.
//!
//! It runs the **real extractor over a real transaction** rather than
//! hand-built events, because the point being proved is a property of on-chain
//! data: that the two legs are indistinguishable on every field the old key
//! looked at. A synthetic pair would only prove that two rows I made different
//! stay different.

use std::path::PathBuf;

use sqlx::PgPool;
use yog_core::application::extraction::rpc::{self, EncodedConfirmedTransactionWithStatusMeta};
use yog_core::application::extraction::{EventExtractor, MeteoraDammV2};
use yog_core::domain::{
    DomainEvent, InsertOutcome, MeteoraDammV2Event, MeteoraDammV2SwapEvent,
    MeteoraDammV2SwapEventRepository,
};
use yog_persistence::PgMeteoraDammV2SwapEventRepository;

/// The mainnet transaction `2qJrr…`: two swaps on the **same pool**, in
/// opposite directions, one signature, one `blockTime`.
///
/// Read from `yog-core`'s fixture directory instead of copied here: its whole
/// value is being the verbatim RPC response, and a second copy would drift
/// from the one the extractor's own tests assert against.
pub(super) fn swap_double_swaps() -> Vec<MeteoraDammV2SwapEvent> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../core/tests/fixtures/damm_v2/swap_double.json");

    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
    let tx: EncodedConfirmedTransactionWithStatusMeta =
        serde_json::from_str(&raw).expect("fixture is not a valid RPC transaction");

    let view = rpc::from_rpc(&tx).expect("fixture is not adaptable");
    let outcome = MeteoraDammV2::new()
        .extract_events(&view)
        .expect("extraction failed on the fixture");

    outcome
        .events
        .into_iter()
        .filter_map(|e| match e {
            DomainEvent::MeteoraDammV2(MeteoraDammV2Event::Swap(s)) => Some(s),
            _ => None,
        })
        .collect()
}

/// Both hops of a routed transaction reach the table.
///
/// Mutation-checked: put migration 041's unique index and this repository's
/// `ON CONFLICT` target back to `(signature, timestamp)`, and it fails with
/// `leg 1 was not written / left: Skipped, right: Inserted` — the defect,
/// reproduced. Without running that mutation the test would be green against
/// the very bug it exists to catch.
#[sqlx::test]
async fn both_legs_of_a_routed_transaction_persist(pool: PgPool) {
    let swaps = swap_double_swaps();
    assert_eq!(swaps.len(), 2, "fixture must carry exactly two swaps");

    // What made the old key collapse them: everything it looked at is equal,
    // and so is the pool — which is why adding `pool_address` to the key would
    // not have helped either.
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

/// Idempotency is preserved: the same event twice still writes one row, and
/// the second attempt now *says so* instead of returning a bare `Ok(())`.
///
/// This is the other half of the contract. A key wide enough to separate two
/// events is only correct if it still collapses a genuine replay.
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

/// The position columns survive the round-trip — a `DEFAULT 0` left in place,
/// or a bind in the wrong slot, would otherwise store zeros that read back as
/// a plausible "first event of its transaction".
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
    // `getTransaction` does not return it; the column exists for the gRPC
    // migration. If this ever fails, the ingestion path started supplying it —
    // which is good news, and makes the ordering guard total.
    assert_eq!(transaction_index, None);
}

/// Storing both hops is only half the fix: the feed must be able to *page*
/// through them. The cursor is `(timestamp, signature, event_index)` — with
/// only the first two it is not a total order over these rows, and a page
/// boundary falling between two hops silently drops the second.
///
/// Mutation-checked: remove the `event_index` clause from the forward
/// predicate in `swap_event.rs` and the second page comes back empty, so the
/// recovered leg is lost again — this time on the read side.
#[sqlx::test]
async fn paging_across_two_hops_of_one_transaction_skips_nothing(pool: PgPool) {
    use yog_core::domain::MeteoraDammV2SwapEventFeed;
    use yog_core::tools::{Cursor, PageDirection};

    let swaps = swap_double_swaps();
    let pool_address = swaps[0].pool_address;
    let repo = PgMeteoraDammV2SwapEventRepository::new(pool.clone());
    for swap in &swaps {
        assert_eq!(repo.insert(swap).await.unwrap(), InsertOutcome::Inserted);
    }

    // One row per page, so the boundary lands exactly between the two hops.
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

/// The backward half of the same guarantee.
///
/// `Prev` navigation is a real API surface, and its predicate is the mirror of
/// the forward one — `<` for `>`, `DESC` for `ASC` on all three keys. A mirror
/// is exactly the kind of thing that looks obviously right and is worth one
/// test anyway: it was written by hand, twice, in two repositories.
///
/// Mutation-checked: drop the `event_index` clause from the backward predicate
/// and the second page comes back empty, same as forward.
#[sqlx::test]
async fn paging_backward_across_two_hops_skips_nothing(pool: PgPool) {
    use yog_core::domain::MeteoraDammV2SwapEventFeed;
    use yog_core::tools::{Cursor, PageDirection, PagePosition};

    let swaps = swap_double_swaps();
    let pool_address = swaps[0].pool_address;
    let repo = PgMeteoraDammV2SwapEventRepository::new(pool.clone());
    for swap in &swaps {
        assert_eq!(repo.insert(swap).await.unwrap(), InsertOutcome::Inserted);
    }

    // Jump to the far end of the list, one row per page, then walk back: the
    // boundary again falls between the two hops.
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

/// The liquidity feed received the same three columns, the same key, the same
/// cursor and the same predicates as the swap feed — and, until this test, no
/// coverage of its own. "Same shape, therefore same behaviour" is the
/// assumption this audit has falsified three times.
///
/// The pair is built by hand rather than extracted, and that is a deliberate
/// difference from the swap tests above: no fixture in the repo carries two
/// liquidity events in one transaction. What needs real data is the *claim
/// about the chain* — that two events genuinely share signature, timestamp and
/// pool — and `swap_double.json` already establishes it. What this test
/// exercises is storage and cursor code, where a hand-built pair proves the
/// same thing.
#[sqlx::test]
async fn liquidity_feed_stores_and_pages_two_hops_of_one_transaction(pool: PgPool) {
    use yog_core::domain::{
        MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventFeed,
        MeteoraDammV2LiquidityEventKind, MeteoraDammV2LiquidityEventRepository,
    };
    use yog_core::tools::{Cursor, PageDirection};
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
