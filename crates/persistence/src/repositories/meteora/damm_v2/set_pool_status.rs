//! Set-pool-status events repository.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{MeteoraDammV2SetPoolStatusEvent, MeteoraDammV2SetPoolStatusEventRepository},
};

use crate::repositories::helper::map_sqlx_error;

pub struct PgMeteoraDammV2SetPoolStatusEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2SetPoolStatusEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2SetPoolStatusEventRepository for PgMeteoraDammV2SetPoolStatusEventRepository {
    async fn insert(&self, event: &MeteoraDammV2SetPoolStatusEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_set_pool_status_events (
                pool_address, signature, status, timestamp
            )
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.status as i16,
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
