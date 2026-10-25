//! `PgProtocolFeeClaimEventRepository` under `yog_api`: the protocol fee claims of a scope
//! from the start of a window, the most recent first, paged without repeat nor gap through
//! shared instants.

use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page,
    domain::{
        EventScope, MeteoraDammV2ClaimProtocolFeeEvent, Protocol, ProtocolFeeClaimEvent,
        ProtocolFeeClaimEventLookup,
    },
};
use yog_persistence::PgProtocolFeeClaimEventRepository;

use super::helpers::{pk, setup_roles_and_connect_as};

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::seconds(seconds)
}

fn sig(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

async fn insert_claim(
    pool: &PgPool,
    address: Pubkey,
    signature: Signature,
    index: i32,
    seconds: i64,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_claim_protocol_fee_events
           (pool_address, signature, token_a_amount, token_b_amount, timestamp, slot,
            event_index, transaction_index)
         VALUES ($1, $2, 1, 2, $3, 9, $4, NULL)",
    )
    .bind(address.to_string())
    .bind(signature.to_string())
    .bind(at(seconds))
    .bind(index)
    .execute(pool)
    .await
    .unwrap();
}

fn keys(page: &Page<ProtocolFeeClaimEvent>) -> Vec<(DateTime<Utc>, String, u16)> {
    page.items
        .iter()
        .map(|ProtocolFeeClaimEvent::MeteoraDammV2(e)| {
            (e.timestamp, e.signature.to_string(), e.event_index)
        })
        .collect()
}

async fn read_all(
    repository: &PgProtocolFeeClaimEventRepository,
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
            Some(Cursor::ProtocolFeeClaimEvent(next)) => cursor = Some(*next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }
    seen
}

/// Mutation: the tie-break dropped, or the cursor compared with `<=` — a claim sharing an
/// instant skipped, or repeated. The order checked is the database's, never a Rust sort.
#[sqlx::test(migrations = "./migrations")]
async fn a_page_follows_through_shared_instants_without_repeat_nor_gap(pool: PgPool) {
    // Four claims at one instant, two of one transaction: only the signature, then the index,
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
        insert_claim(&pool, pk(1), sig(seed), index, seconds).await;
    }
    let repository =
        PgProtocolFeeClaimEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

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
}

/// Mutation: the pool filter, the protocol dispatch or the window's start dropped — another
/// pool's claim, a DLMM page, or a claim before the window, listed.
#[sqlx::test(migrations = "./migrations")]
async fn a_scope_and_its_window_read_only_their_claims(pool: PgPool) {
    insert_claim(&pool, pk(1), sig(1), 0, 10).await;
    insert_claim(&pool, pk(2), sig(2), 0, 20).await;
    insert_claim(&pool, pk(1), sig(3), 0, -120).await;
    let repository =
        PgProtocolFeeClaimEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

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

/// Mutation: a column read into another field — two of its values swapped, or one read for
/// another.
#[sqlx::test(migrations = "./migrations")]
async fn a_claim_comes_back_as_written(pool: PgPool) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_claim_protocol_fee_events
               (pool_address, signature, timestamp, slot, event_index, transaction_index,
                token_a_amount, token_b_amount)
             VALUES ($1, $2, $3, 9, 0, NULL, 11, 12)",
    )
    .bind(pk(1).to_string())
    .bind(sig(1).to_string())
    .bind(at(10))
    .execute(&pool)
    .await
    .unwrap();
    let repository =
        PgProtocolFeeClaimEventRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let page = repository
        .list_events(EventScope::All, at(-60), None, 50)
        .await
        .unwrap();

    let expected = MeteoraDammV2ClaimProtocolFeeEvent {
        pool_address: pk(1),
        signature: sig(1),
        timestamp: at(10),
        slot: 9,
        transaction_index: None,
        event_index: 0,
        token_a_amount: 11,
        token_b_amount: 12,
    };
    let read: Vec<_> = page
        .items
        .iter()
        .map(|ProtocolFeeClaimEvent::MeteoraDammV2(e)| format!("{e:?}"))
        .collect();
    assert_eq!(read, [format!("{expected:?}")]);
}
