use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
use yog_core::domain::MeteoraDammV2InitializePoolEventRepository;

use yog_core::domain::MeteoraDammV2InitializePoolEvent;
use yog_persistence::PgMeteoraDammV2InitializePoolEventRepository;

use super::helpers::{pk, sg, ts};

// ── initialize_pool: u128 NUMERIC + BYTEA fee blob + SMALLINT ───────

#[sqlx::test]
async fn initialize_pool_preserves_numeric_bytea_smallint(pool: PgPool) {
    let repo = PgMeteoraDammV2InitializePoolEventRepository::new(pool.clone());
    let fee_blob = vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x2a];
    assert_eq!(
        repo.insert(&MeteoraDammV2InitializePoolEvent {
            pool_address: pk(1),
            signature: sg(),
            timestamp: ts(),
            slot: 1,
            transaction_index: None,
            event_index: 0,
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
        .unwrap(),
        InsertOutcome::Inserted
    );

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
