//! Split-position events repository: inserts position-to-position transfers.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        InsertOutcome, MeteoraDammV2SplitPositionEvent, MeteoraDammV2SplitPositionEventRepository,
    },
};

use crate::repositories::helper::{convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error};

pub struct PgMeteoraDammV2SplitPositionEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2SplitPositionEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2SplitPositionEventRepository for PgMeteoraDammV2SplitPositionEventRepository {
    async fn insert(
        &self,
        event: &MeteoraDammV2SplitPositionEvent,
    ) -> RepositoryResult<InsertOutcome> {
        let a = &event.amounts;
        let f = &event.first_position_after;
        let s = &event.second_position_after;
        let n = &event.numerators;

        let result = sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_split_position_events (
                pool_address, signature,
                first_owner, second_owner, first_position, second_position,
                current_sqrt_price,
                split_permanent_locked_liquidity, split_unlocked_liquidity,
                split_vested_liquidity, split_fee_a, split_fee_b,
                split_reward_0, split_reward_1,
                first_unlocked_liquidity, first_permanent_locked_liquidity,
                first_vested_liquidity, first_fee_a, first_fee_b,
                first_reward_0, first_reward_1,
                second_unlocked_liquidity, second_permanent_locked_liquidity,
                second_vested_liquidity, second_fee_a, second_fee_b,
                second_reward_0, second_reward_1,
                num_unlocked_liquidity, num_permanent_locked_liquidity,
                num_fee_a, num_fee_b, num_reward_0, num_reward_1,
                num_inner_vesting_liquidity,
                timestamp,
                slot, event_index, transaction_index
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10, $11, $12, $13, $14,
                $15, $16, $17, $18, $19, $20, $21,
                $22, $23, $24, $25, $26, $27, $28,
                $29, $30, $31, $32, $33, $34, $35,
                $36,
                $37, $38, $39
            )
            ON CONFLICT (signature, event_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.first_owner.to_string(),
            event.second_owner.to_string(),
            event.first_position.to_string(),
            event.second_position.to_string(),
            convert_u128_to_bigdecimal(event.current_sqrt_price, "current_sqrt_price"),
            // what moved
            convert_u128_to_bigdecimal(a.permanent_locked_liquidity, "split_permanent_locked"),
            convert_u128_to_bigdecimal(a.unlocked_liquidity, "split_unlocked"),
            convert_u128_to_bigdecimal(a.vested_liquidity, "split_vested"),
            convert_u64_to_i64(a.fee_a, "split_fee_a")?,
            convert_u64_to_i64(a.fee_b, "split_fee_b")?,
            convert_u64_to_i64(a.reward_0, "split_reward_0")?,
            convert_u64_to_i64(a.reward_1, "split_reward_1")?,
            // first position, after
            convert_u128_to_bigdecimal(f.unlocked_liquidity, "first_unlocked"),
            convert_u128_to_bigdecimal(f.permanent_locked_liquidity, "first_permanent_locked"),
            convert_u128_to_bigdecimal(f.vested_liquidity, "first_vested"),
            convert_u64_to_i64(f.fee_a, "first_fee_a")?,
            convert_u64_to_i64(f.fee_b, "first_fee_b")?,
            convert_u64_to_i64(f.reward_0, "first_reward_0")?,
            convert_u64_to_i64(f.reward_1, "first_reward_1")?,
            // second position, after
            convert_u128_to_bigdecimal(s.unlocked_liquidity, "second_unlocked"),
            convert_u128_to_bigdecimal(s.permanent_locked_liquidity, "second_permanent_locked"),
            convert_u128_to_bigdecimal(s.vested_liquidity, "second_vested"),
            convert_u64_to_i64(s.fee_a, "second_fee_a")?,
            convert_u64_to_i64(s.fee_b, "second_fee_b")?,
            convert_u64_to_i64(s.reward_0, "second_reward_0")?,
            convert_u64_to_i64(s.reward_1, "second_reward_1")?,
            // requested fractions — u32 widened to i64, lossless
            i64::from(n.unlocked_liquidity),
            i64::from(n.permanent_locked_liquidity),
            i64::from(n.fee_a),
            i64::from(n.fee_b),
            i64::from(n.reward_0),
            i64::from(n.reward_1),
            i64::from(n.inner_vesting_liquidity),
            event.timestamp,
            convert_u64_to_i64(event.slot, "slot")?,
            i32::from(event.event_index),
            event.transaction_index.map(i64::from),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(InsertOutcome::from_rows_affected(result.rows_affected()))
    }
}
