//! Close-position events repository: inserts position closures.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{MeteoraDammV2ClosePositionEvent, MeteoraDammV2ClosePositionEventRepository},
};

use crate::repositories::helper::map_sqlx_error;

pub struct PgMeteoraDammV2ClosePositionEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2ClosePositionEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2ClosePositionEventRepository for PgMeteoraDammV2ClosePositionEventRepository {
    async fn insert(&self, event: &MeteoraDammV2ClosePositionEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_close_position_events (
                pool_address, signature,
                owner, position, position_nft_mint,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.owner.to_string(),
            event.position.to_string(),
            event.position_nft_mint.to_string(),
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
