use sqlx::PgPool;
use yog_core::domain::InsertOutcome;
use yog_core::domain::MeteoraDammV2FundRewardEventRepository;

use yog_core::domain::MeteoraDammV2FundRewardEvent;
use yog_persistence::PgMeteoraDammV2FundRewardEventRepository;

use super::helpers::{pk, sg, ts};

// ── fund_reward: persists, idempotent, u128 Q64.64 rates survive ─────

#[sqlx::test]
async fn fund_reward_preserves_q64_rates(pool: PgPool) {
    let repo = PgMeteoraDammV2FundRewardEventRepository::new(pool.clone());
    // Real fixture values (damm_v2/initialize_reward.json): a slot's first
    // funding, so pre_reward_rate is 0 and post = (amount << 64) / 604800.
    const POST_RATE: u128 = 3_050_056_890_494_304_169_312_169;
    let event = MeteoraDammV2FundRewardEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        slot: 1,
        transaction_index: None,
        event_index: 0,
        funder: pk(2),
        mint_reward: pk(3),
        reward_index: 0,
        amount: 100_000_000_000,
        transfer_fee_excluded_amount_in: 100_000_000_000,
        reward_duration_end: 1_785_727_188,
        pre_reward_rate: 0,
        post_reward_rate: POST_RATE,
    };

    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Inserted);
    // Same (signature, reward_index, timestamp) again — ON CONFLICT DO NOTHING.
    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Skipped);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_fund_reward_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, event_index, timestamp) must not insert twice"
    );

    // A second slot funded by the same transaction is a distinct event — a
    // second `emit_cpi!`, hence a second `event_index`. Since migration 041 that
    // is what the key looks at, not `reward_index`.
    assert_eq!(
        repo.insert(&MeteoraDammV2FundRewardEvent {
            reward_index: 1,
            event_index: 1,
            ..event.clone()
        })
        .await
        .unwrap(),
        InsertOutcome::Inserted
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_fund_reward_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "a second slot in the same tx must insert");

    // The Q64.64 rate is a 25-digit u128: NUMERIC(39,0) must return it to the
    // last digit. Any silent narrowing to f64/i64 would corrupt it here.
    let (pre, post, end): (String, String, i64) = sqlx::query_as(
        "SELECT pre_reward_rate::TEXT, post_reward_rate::TEXT, reward_duration_end \
         FROM meteora_damm_v2_fund_reward_events WHERE reward_index = 0",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pre, "0");
    assert_eq!(post, POST_RATE.to_string());
    assert_eq!(end, 1_785_727_188);
}
