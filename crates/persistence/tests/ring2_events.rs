//! Integration tests for the ring-2 DAMM v2 event repositories.
//!
//! Gated behind the `integration-tests` feature: each test gets an isolated
//! Postgres database (via `sqlx::test`) with the migrations applied. The CI
//! job `test-integration` runs them; a plain `cargo test` skips them.
//!
//! These repos are write-only (no read method to assert against), so they
//! have no `rows_tests.rs` unit coverage and the SQL is only checked at
//! compile time by `sqlx::query!`. These tests close the runtime gap: that an
//! `insert` actually persists, that the type conversions survive a round trip
//! (u128 → NUMERIC(39,0) with no precision loss, fee blobs → BYTEA, u8 →
//! SMALLINT), and that the `ON CONFLICT (signature, timestamp) DO NOTHING`
//! idempotency guard holds.

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::PgPool;

use yog_core::domain::{
    MeteoraDammV2ClosePositionEvent, MeteoraDammV2ClosePositionEventRepository,
    MeteoraDammV2CreatePositionEvent, MeteoraDammV2CreatePositionEventRepository,
    MeteoraDammV2InitializePoolEvent, MeteoraDammV2InitializePoolEventRepository,
    MeteoraDammV2LockPositionEvent, MeteoraDammV2LockPositionEventRepository,
    MeteoraDammV2PermanentLockPositionEvent, MeteoraDammV2PermanentLockPositionEventRepository,
    MeteoraDammV2SetPoolStatusEvent, MeteoraDammV2SetPoolStatusEventRepository,
    MeteoraDammV2UpdatePoolFeesEvent, MeteoraDammV2UpdatePoolFeesEventRepository,
};
use yog_persistence::{
    PgMeteoraDammV2ClosePositionEventRepository, PgMeteoraDammV2CreatePositionEventRepository,
    PgMeteoraDammV2InitializePoolEventRepository, PgMeteoraDammV2LockPositionEventRepository,
    PgMeteoraDammV2PermanentLockPositionEventRepository,
    PgMeteoraDammV2SetPoolStatusEventRepository, PgMeteoraDammV2UpdatePoolFeesEventRepository,
};

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}
fn sg() -> Signature {
    Signature::from([7u8; 64])
}
fn ts() -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000, 0).unwrap()
}

// ── create_position: round trip + idempotency ───────────────────────

#[sqlx::test]
async fn create_position_inserts_and_is_idempotent(pool: PgPool) {
    let repo = PgMeteoraDammV2CreatePositionEventRepository::new(pool.clone());
    let event = MeteoraDammV2CreatePositionEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        owner: pk(2),
        position: pk(3),
        position_nft_mint: pk(4),
    };

    repo.insert(&event).await.unwrap();
    // Same (signature, timestamp) again — ON CONFLICT DO NOTHING.
    repo.insert(&event).await.unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_create_position_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, timestamp) must not insert twice"
    );

    let nft: String = sqlx::query_scalar(
        "SELECT position_nft_mint FROM meteora_damm_v2_create_position_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(nft, pk(4).to_string());
}

// ── close_position: persists ─────────────────────────────────────────

#[sqlx::test]
async fn close_position_inserts(pool: PgPool) {
    let repo = PgMeteoraDammV2ClosePositionEventRepository::new(pool.clone());
    repo.insert(&MeteoraDammV2ClosePositionEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        owner: pk(2),
        position: pk(3),
        position_nft_mint: pk(4),
    })
    .await
    .unwrap();

    let owner: String =
        sqlx::query_scalar("SELECT owner FROM meteora_damm_v2_close_position_events LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owner, pk(2).to_string());
}

// ── lock_position: u128 NUMERIC precision at the u128::MAX boundary ──

#[sqlx::test]
async fn lock_position_preserves_u128_and_u16(pool: PgPool) {
    let repo = PgMeteoraDammV2LockPositionEventRepository::new(pool.clone());
    repo.insert(&MeteoraDammV2LockPositionEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        position: pk(2),
        owner: pk(3),
        vesting: pk(4),
        cliff_point: 426_006_082,
        period_frequency: 1,
        // u128::MAX is exactly 39 digits — the NUMERIC(39, 0) boundary.
        cliff_unlock_liquidity: u128::MAX,
        liquidity_per_period: 0,
        number_of_period: 65_535, // u16::MAX — must survive the INTEGER column
    })
    .await
    .unwrap();

    let cliff: String = sqlx::query_scalar(
        "SELECT cliff_unlock_liquidity::text FROM meteora_damm_v2_lock_position_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        cliff,
        u128::MAX.to_string(),
        "u128::MAX lost precision in NUMERIC"
    );

    let n: i32 = sqlx::query_scalar(
        "SELECT number_of_period FROM meteora_damm_v2_lock_position_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 65_535);
}

// ── permanent_lock_position: both u128 fields round-trip exactly ────

#[sqlx::test]
async fn permanent_lock_position_preserves_u128(pool: PgPool) {
    let repo = PgMeteoraDammV2PermanentLockPositionEventRepository::new(pool.clone());
    let lock: u128 = 38_221_888_425_530_974_168_949_248_950_912;
    let total: u128 = 76_443_776_851_061_948_337_898_497_901_824;
    repo.insert(&MeteoraDammV2PermanentLockPositionEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        position: pk(2),
        lock_liquidity_amount: lock,
        total_permanent_locked_liquidity: total,
    })
    .await
    .unwrap();

    let (got_lock, got_total): (String, String) = {
        let l: String = sqlx::query_scalar(
            "SELECT lock_liquidity_amount::text FROM meteora_damm_v2_permanent_lock_position_events LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let t: String = sqlx::query_scalar(
            "SELECT total_permanent_locked_liquidity::text FROM meteora_damm_v2_permanent_lock_position_events LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        (l, t)
    };
    assert_eq!(got_lock, lock.to_string());
    assert_eq!(got_total, total.to_string());
}

// ── initialize_pool: u128 NUMERIC + BYTEA fee blob + SMALLINT ───────

#[sqlx::test]
async fn initialize_pool_preserves_numeric_bytea_smallint(pool: PgPool) {
    let repo = PgMeteoraDammV2InitializePoolEventRepository::new(pool.clone());
    let fee_blob = vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x2a];
    repo.insert(&MeteoraDammV2InitializePoolEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        token_a_mint: pk(2),
        token_b_mint: pk(3),
        creator: pk(4),
        payer: pk(5),
        alpha_vault: pk(6),
        sqrt_min_price: 1,
        sqrt_max_price: u128::MAX,
        sqrt_price: 79_226_673_521_066_979_257_578_248_091,
        liquidity: 1_000_000_000_000,
        activation_type: 1,
        activation_point: 250,
        collect_fee_mode: 2,
        pool_type: 3,
        token_a_flag: 1,
        token_b_flag: 0,
        token_a_amount: 10,
        token_b_amount: 20,
        total_amount_a: 10,
        total_amount_b: 20,
        pool_fees_raw: fee_blob.clone(),
    })
    .await
    .unwrap();

    let max_price: String = sqlx::query_scalar(
        "SELECT sqrt_max_price::text FROM meteora_damm_v2_initialize_pool_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(max_price, u128::MAX.to_string());

    let raw: Vec<u8> = sqlx::query_scalar(
        "SELECT pool_fees_raw FROM meteora_damm_v2_initialize_pool_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        raw, fee_blob,
        "fee blob must round-trip through BYTEA byte-for-byte"
    );

    let pool_type: i16 =
        sqlx::query_scalar("SELECT pool_type FROM meteora_damm_v2_initialize_pool_events LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pool_type, 3);
}

// ── set_pool_status: SMALLINT ───────────────────────────────────────

#[sqlx::test]
async fn set_pool_status_inserts(pool: PgPool) {
    let repo = PgMeteoraDammV2SetPoolStatusEventRepository::new(pool.clone());
    repo.insert(&MeteoraDammV2SetPoolStatusEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        status: 1,
    })
    .await
    .unwrap();

    let status: i16 =
        sqlx::query_scalar("SELECT status FROM meteora_damm_v2_set_pool_status_events LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, 1);
}

// ── update_pool_fees: BYTEA params blob ─────────────────────────────

#[sqlx::test]
async fn update_pool_fees_preserves_bytea(pool: PgPool) {
    let repo = PgMeteoraDammV2UpdatePoolFeesEventRepository::new(pool.clone());
    let params = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 0];
    repo.insert(&MeteoraDammV2UpdatePoolFeesEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        operator: pk(2),
        params_raw: params.clone(),
    })
    .await
    .unwrap();

    let raw: Vec<u8> = sqlx::query_scalar(
        "SELECT params_raw FROM meteora_damm_v2_update_pool_fees_events LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(raw, params);
}
