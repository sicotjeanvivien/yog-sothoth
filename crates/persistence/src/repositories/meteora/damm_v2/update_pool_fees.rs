//! Update-pool-fees events repository.
//!
//! Write-only — the indexer is the sole consumer today. The fee parameters
//! are persisted as a raw, undecoded borsh blob (`params_raw`).

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        InsertOutcome, MeteoraDammV2UpdatePoolFeesEvent, MeteoraDammV2UpdatePoolFeesEventRepository,
    },
};

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};

pub struct PgMeteoraDammV2UpdatePoolFeesEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2UpdatePoolFeesEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2UpdatePoolFeesEventRepository for PgMeteoraDammV2UpdatePoolFeesEventRepository {
    async fn insert(
        &self,
        event: &MeteoraDammV2UpdatePoolFeesEvent,
    ) -> RepositoryResult<InsertOutcome> {
        let result = sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_update_pool_fees_events (
                pool_address, signature, operator, params_raw, timestamp,
                slot, event_index, transaction_index
            )
            VALUES ($1, $2, $3, $4, $5,
                $6, $7, $8
            )
            ON CONFLICT (signature, event_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.operator.to_string(),
            event.params_raw,
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
