use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    domain::{Pool, PoolRepository},
    RepositoryResult,
};

use crate::infra::db::repository_utils::map_sqlx_error;

pub(crate) struct PgPoolRepository {
    pool: PgPool,
}

impl PgPoolRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolRepository for PgPoolRepository {
    async fn upsert(&self, pool: &Pool) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO pools
                (pool_address, protocol, token_a_mint, token_b_mint,
                 first_seen_at, last_seen_at)
            VALUES ($1, $2, $3, $4, $5, $5)
            ON CONFLICT (pool_address) DO UPDATE
                SET last_seen_at = EXCLUDED.last_seen_at
            "#,
            pool.pool_address.to_string(),
            pool.protocol.as_str(),
            pool.token_a_mint.to_string(),
            pool.token_b_mint.to_string(),
            pool.last_seen_at,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
