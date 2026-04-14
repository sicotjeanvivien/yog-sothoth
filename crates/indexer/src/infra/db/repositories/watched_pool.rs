use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    domain::{WatchedPool, WatchedPoolRepository},
    CoreError, CoreResult,
};

use crate::infra::db::{convert_string_to_pubkey, parse_string_to_protocol};

pub(crate) struct PgWatchedPoolRepository {
    pool: PgPool,
}

impl PgWatchedPoolRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WatchedPoolRepository for PgWatchedPoolRepository {
    async fn add(&self, watched_pool: &WatchedPool) -> CoreResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO watched_pools
                (pool_address, protocol, token_a_mint, token_b_mint, token_a_decimals, token_b_decimals)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (pool_address) DO NOTHING
            "#,
            watched_pool.pool_address.to_string(),
            watched_pool.protocol.as_str(),
            watched_pool.token_a_mint.to_string(),
            watched_pool.token_b_mint.to_string(),
            watched_pool.token_a_decimals as i16,
            watched_pool.token_b_decimals as i16,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::ParseError {
            signature: String::new(),
            reason: format!("db insert watched_pool: {e}"),
        })?;

        Ok(())
    }

    async fn exists(&self, address: &str) -> CoreResult<bool> {
        let result = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM watched_pools WHERE pool_address = $1)",
            address
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::ParseError {
            signature: String::new(),
            reason: format!("db exists watched_pool: {e}"),
        })?;

        Ok(result.unwrap_or(false))
    }

    async fn find_all(&self) -> CoreResult<Vec<WatchedPool>> {
        let rows = sqlx::query!(
            r#"
            SELECT pool_address, protocol, token_a_mint, token_b_mint,
                   token_a_decimals, token_b_decimals, added_at
            FROM watched_pools
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::ParseError {
            signature: String::new(),
            reason: format!("db find_all watched_pool: {e}"),
        })?;

        let pools = rows
            .into_iter()
            .map(|row| {
                Ok(WatchedPool {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    protocol: parse_string_to_protocol(row.protocol, "protocol")?,
                    token_a_mint: convert_string_to_pubkey(row.token_a_mint, "token_a_mint")?,
                    token_b_mint: convert_string_to_pubkey(row.token_b_mint, "token_b_mint")?,
                    token_a_decimals: row.token_a_decimals as u8,
                    token_b_decimals: row.token_b_decimals as u8,
                    added_at: row.added_at,
                })
            })
            .collect::<CoreResult<Vec<_>>>()?;

        Ok(pools)
    }

    async fn remove(&self, pool_address: &str) -> CoreResult<()> {
        sqlx::query!(
            "DELETE FROM watched_pools WHERE pool_address = $1",
            pool_address
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::ParseError {
            signature: String::new(),
            reason: format!("db remove watched_pool: {e}"),
        })?;

        Ok(())
    }
}
