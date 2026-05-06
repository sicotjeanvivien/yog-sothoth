use std::str::FromStr;

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{Protocol, WatchedPool, WatchedPoolRepository},
};

use crate::repository_utils::{convert_string_to_pubkey, map_sqlx_error};

pub struct PgWatchedPoolRepository {
    pool: PgPool,
}

impl PgWatchedPoolRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WatchedPoolRepository for PgWatchedPoolRepository {
    async fn add(&self, watched_pool: &WatchedPool) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO watched_pools
                (pool_address, protocol, active, added_at, note)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (pool_address) DO NOTHING
            "#,
            watched_pool.pool_address.to_string(),
            watched_pool.protocol.as_str(),
            watched_pool.active,
            watched_pool.added_at,
            watched_pool.note,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn exists(&self, address: &str) -> RepositoryResult<bool> {
        let result = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM watched_pools WHERE pool_address = $1)",
            address
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(result.unwrap_or(false))
    }

    async fn find_all(&self) -> RepositoryResult<Vec<WatchedPool>> {
        let rows = sqlx::query!(
            r#"
            SELECT pool_address, protocol, active, added_at, note
            FROM watched_pools
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| {
                Ok(WatchedPool {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    protocol: Protocol::from_str(&row.protocol).map_err(|e| {
                        RepositoryError::Integrity(format!("invalid protocol: {e}"))
                    })?,
                    active: row.active,
                    added_at: row.added_at,
                    note: row.note,
                })
            })
            .collect()
    }

    async fn remove(&self, pool_address: &str) -> RepositoryResult<()> {
        sqlx::query!(
            "DELETE FROM watched_pools WHERE pool_address = $1",
            pool_address
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
