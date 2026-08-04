//! Permanent-lock-position events repository: inserts permanent liquidity locks.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        InsertOutcome, MeteoraDammV2PermanentLockPositionEvent,
        MeteoraDammV2PermanentLockPositionEventRepository,
    },
};

use crate::repositories::helper::{convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error};

pub struct PgMeteoraDammV2PermanentLockPositionEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2PermanentLockPositionEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2PermanentLockPositionEventRepository
    for PgMeteoraDammV2PermanentLockPositionEventRepository
{
    async fn insert(
        &self,
        event: &MeteoraDammV2PermanentLockPositionEvent,
    ) -> RepositoryResult<InsertOutcome> {
        let result = sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_permanent_lock_position_events (
                pool_address, signature,
                position, lock_liquidity_amount, total_permanent_locked_liquidity,
                timestamp,
                slot, event_index, transaction_index
            )
            VALUES ($1, $2, $3, $4, $5, $6,
                $7, $8, $9
            )
            ON CONFLICT (signature, event_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.position.to_string(),
            convert_u128_to_bigdecimal(event.lock_liquidity_amount, "lock_liquidity_amount"),
            convert_u128_to_bigdecimal(
                event.total_permanent_locked_liquidity,
                "total_permanent_locked_liquidity"
            ),
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
