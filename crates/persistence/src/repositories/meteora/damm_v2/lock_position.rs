//! Lock-position events repository: inserts position vesting locks.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{MeteoraDammV2LockPositionEvent, MeteoraDammV2LockPositionEventRepository},
};

use crate::repositories::helper::{convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error};

pub struct PgMeteoraDammV2LockPositionEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2LockPositionEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2LockPositionEventRepository for PgMeteoraDammV2LockPositionEventRepository {
    async fn insert(&self, event: &MeteoraDammV2LockPositionEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_lock_position_events (
                pool_address, signature,
                position, owner, vesting,
                cliff_point, period_frequency,
                cliff_unlock_liquidity, liquidity_per_period, number_of_period,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.position.to_string(),
            event.owner.to_string(),
            event.vesting.to_string(),
            convert_u64_to_i64(event.cliff_point, "cliff_point")?,
            convert_u64_to_i64(event.period_frequency, "period_frequency")?,
            convert_u128_to_bigdecimal(event.cliff_unlock_liquidity, "cliff_unlock_liquidity"),
            convert_u128_to_bigdecimal(event.liquidity_per_period, "liquidity_per_period"),
            event.number_of_period as i32,
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
