//! `PgPoolCreationEventRepository` under `yog_api`: the pool creations from the start of a
//! window, the most recent first, paged without repeat nor gap through shared instants.

use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_signature::Signature;
use sqlx::PgPool;
use yog_core::{
    Cursor,
    domain::{EventScope, PoolCreationEvent, PoolCreationEventLookup, Protocol},
};
use yog_persistence::PgPoolCreationEventRepository;

use super::helpers::{pk, setup_roles_and_connect_as};

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::seconds(seconds)
}

async fn insert_creation(pool: &PgPool, seed: u8, seconds: i64) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_initialize_pool_events
           (pool_address, signature, token_a_mint, token_b_mint, creator, payer, alpha_vault,
            sqrt_min_price, sqrt_max_price, sqrt_price, liquidity, activation_type,
            activation_point, collect_fee_mode, pool_type, token_a_flag, token_b_flag,
            token_a_amount, token_b_amount, total_amount_a, total_amount_b, pool_fees_raw,
            timestamp, slot, event_index, transaction_index)
         VALUES ($1, $2, $3, $3, $3, $3, $3, 1, 2, 3, 4, 1, 5, 0, 0, 0, 0, 6, 7, 8, 9, '\\x00',
                 $4, 9, 0, NULL)",
    )
    .bind(pk(seed).to_string())
    .bind(Signature::from([seed; 64]).to_string())
    .bind(pk(100).to_string())
    .bind(at(seconds))
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the tie-break dropped, the cursor compared with `<=`, or the window's start or
/// the protocol dispatch dropped — a creation of a shared instant skipped or repeated, one
/// before the window listed, or a DLMM page.
#[sqlx::test(migrations = "./migrations")]
async fn a_page_follows_through_shared_instants_from_the_windows_start(pool: PgPool) {
    for (seed, seconds) in [(1, 10), (2, 10), (3, 10), (4, 5), (5, -120)] {
        insert_creation(&pool, seed, seconds).await;
    }
    let repository =
        PgPoolCreationEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = repository
            .list_events(EventScope::All, at(-60), cursor.take(), 2)
            .await
            .unwrap();
        seen.extend(
            page.items
                .iter()
                .map(|PoolCreationEvent::MeteoraDammV2(e)| e.pool_address),
        );
        match page.next_cursor {
            Some(Cursor::PoolCreationEvent(next)) => cursor = Some(*next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }
    let dlmm = repository
        .list_events(
            EventScope::Protocol(Protocol::MeteoraDlmm),
            at(-60),
            None,
            50,
        )
        .await
        .unwrap();

    // The three of one instant, by signature descending, then the earlier one.
    let mut tied: Vec<(String, _)> = [1u8, 2, 3]
        .map(|seed| (Signature::from([seed; 64]).to_string(), pk(seed)))
        .to_vec();
    tied.sort_by(|a, b| b.0.cmp(&a.0));
    let expected: Vec<_> = tied
        .into_iter()
        .map(|(_, address)| address)
        .chain([pk(4)])
        .collect();
    assert_eq!(seen, expected);
    assert!(dlmm.items.is_empty());
}
