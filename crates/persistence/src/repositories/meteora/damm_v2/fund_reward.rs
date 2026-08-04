//! Fund-reward events repository: inserts farm fundings and their emission rate.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{InsertOutcome, MeteoraDammV2FundRewardEvent, MeteoraDammV2FundRewardEventRepository},
};

use crate::repositories::helper::{convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error};

pub struct PgMeteoraDammV2FundRewardEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2FundRewardEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2FundRewardEventRepository for PgMeteoraDammV2FundRewardEventRepository {
    async fn insert(
        &self,
        event: &MeteoraDammV2FundRewardEvent,
    ) -> RepositoryResult<InsertOutcome> {
        let result = sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_fund_reward_events (
                pool_address, signature,
                funder, mint_reward, reward_index,
                amount, transfer_fee_excluded_amount_in, reward_duration_end,
                pre_reward_rate, post_reward_rate,
                timestamp,
                slot, event_index, transaction_index
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                $12, $13, $14
            )
            ON CONFLICT (signature, event_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.funder.to_string(),
            event.mint_reward.to_string(),
            event.reward_index as i16,
            convert_u64_to_i64(event.amount, "amount")?,
            convert_u64_to_i64(
                event.transfer_fee_excluded_amount_in,
                "transfer_fee_excluded_amount_in"
            )?,
            convert_u64_to_i64(event.reward_duration_end, "reward_duration_end")?,
            // Q64.64 rates — stored unscaled, exactly as emitted.
            convert_u128_to_bigdecimal(event.pre_reward_rate, "pre_reward_rate"),
            convert_u128_to_bigdecimal(event.post_reward_rate, "post_reward_rate"),
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
