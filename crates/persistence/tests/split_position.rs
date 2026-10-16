use sqlx::PgPool;
use yog_core::domain::InsertOutcome;

use yog_core::domain::{
    MeteoraDammV2SplitAmounts, MeteoraDammV2SplitNumerators, MeteoraDammV2SplitPositionEvent,
    MeteoraDammV2SplitPositionEventRepository, MeteoraDammV2SplitPositionState,
};
use yog_persistence::PgMeteoraDammV2SplitPositionEventRepository;

use super::helpers::{pk, sg, ts};

#[sqlx::test]
async fn split_position_preserves_every_bucket(pool: PgPool) {
    let repo = PgMeteoraDammV2SplitPositionEventRepository::new(pool.clone());
    // ⚠️ A distinct value per field: the three u128 buckets come in different orders on the
    // wire, so only differing values catch a mix-up.
    const BIG: u128 = u128::MAX / 3;
    let event = MeteoraDammV2SplitPositionEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        slot: 1,
        transaction_index: None,
        event_index: 0,
        first_owner: pk(2),
        second_owner: pk(3),
        first_position: pk(4),
        second_position: pk(5),
        current_sqrt_price: BIG,
        amounts: MeteoraDammV2SplitAmounts {
            permanent_locked_liquidity: 11,
            unlocked_liquidity: 12,
            vested_liquidity: 13,
            fee_a: 14,
            fee_b: 15,
            reward_0: 16,
            reward_1: 17,
        },
        first_position_after: MeteoraDammV2SplitPositionState {
            unlocked_liquidity: 21,
            permanent_locked_liquidity: 22,
            vested_liquidity: 23,
            fee_a: 24,
            fee_b: 25,
            reward_0: 26,
            reward_1: 27,
        },
        second_position_after: MeteoraDammV2SplitPositionState {
            unlocked_liquidity: 31,
            permanent_locked_liquidity: 32,
            vested_liquidity: 33,
            fee_a: 34,
            fee_b: 35,
            reward_0: 36,
            reward_1: 37,
        },
        numerators: MeteoraDammV2SplitNumerators {
            unlocked_liquidity: 1_000_000_000,
            permanent_locked_liquidity: 42,
            fee_a: 43,
            fee_b: 44,
            reward_0: 45,
            reward_1: 46,
            inner_vesting_liquidity: 47,
        },
    };

    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Inserted);
    assert_eq!(repo.insert(&event).await.unwrap(), InsertOutcome::Skipped);

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_split_position_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "duplicate (signature, event_index, timestamp) must not insert twice"
    );

    // One transaction can split one source toward several targets: `event_index` tells them apart.
    assert_eq!(
        repo.insert(&MeteoraDammV2SplitPositionEvent {
            second_position: pk(6),
            event_index: 1,
            ..event.clone()
        })
        .await
        .unwrap(),
        InsertOutcome::Inserted
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM meteora_damm_v2_split_position_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2, "a second split target must insert");

    // u128 through NUMERIC(39,0): exact to the last digit.
    let sqrt: String = sqlx::query_scalar(
        "SELECT current_sqrt_price::TEXT FROM meteora_damm_v2_split_position_events \
         WHERE second_position = $1",
    )
    .bind(pk(5).to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sqrt, BIG.to_string());

    // Mutation: an `amounts` / `*_position_after` binding swapped.
    let (sp, su, sv, fu, fp, su2, sp2): (String, String, String, String, String, String, String) =
        sqlx::query_as(
            "SELECT split_permanent_locked_liquidity::TEXT, split_unlocked_liquidity::TEXT, \
                    split_vested_liquidity::TEXT, first_unlocked_liquidity::TEXT, \
                    first_permanent_locked_liquidity::TEXT, second_unlocked_liquidity::TEXT, \
                    second_permanent_locked_liquidity::TEXT \
             FROM meteora_damm_v2_split_position_events WHERE second_position = $1",
        )
        .bind(pk(5).to_string())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        (
            sp.as_str(),
            su.as_str(),
            sv.as_str(),
            fu.as_str(),
            fp.as_str(),
            su2.as_str(),
            sp2.as_str()
        ),
        ("11", "12", "13", "21", "22", "31", "32")
    );

    // u32 numerator at its on-chain maximum survives the widening to BIGINT.
    let num: i64 = sqlx::query_scalar(
        "SELECT num_unlocked_liquidity FROM meteora_damm_v2_split_position_events \
         WHERE second_position = $1",
    )
    .bind(pk(5).to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(num, 1_000_000_000);
}
