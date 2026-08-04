//! Create-position events repository: inserts new positions.
//!
//! Write-only — the indexer is the sole consumer today. No read-side
//! (no SELECT, no row mapping) until an API endpoint needs it.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        InsertOutcome, MeteoraDammV2CreatePositionEvent, MeteoraDammV2CreatePositionEventRepository,
    },
};

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};

pub struct PgMeteoraDammV2CreatePositionEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2CreatePositionEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2CreatePositionEventRepository for PgMeteoraDammV2CreatePositionEventRepository {
    async fn insert(
        &self,
        event: &MeteoraDammV2CreatePositionEvent,
    ) -> RepositoryResult<InsertOutcome> {
        let result = sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_create_position_events (
                pool_address, signature,
                owner, position, position_nft_mint,
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
            event.owner.to_string(),
            event.position.to_string(),
            event.position_nft_mint.to_string(),
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
