mod rows;

use crate::repositories::helper::{convert_decimal_to_bigdecimal, map_sqlx_error, parse_protocol};
use async_trait::async_trait;
use rows::PoolRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        Pool, PoolCatalog, PoolCounts, PoolRegistryProperties, PoolRepository, ProtocolPools,
    },
};

pub struct PgPoolRepository {
    pool: PgPool,
}

impl PgPoolRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolRepository for PgPoolRepository {
    /// Never lowers `last_seen_at`, nor stores it past `NOW()`.
    /// ⚠️ Not `ON CONFLICT … DO UPDATE`: its target arbitrates one of the two unique indexes, and
    /// a concurrent insert fails or deadlocks on the other; untargeted, `DO NOTHING` arbitrates both.
    async fn upsert(&self, pool: &Pool) -> RepositoryResult<()> {
        let updated = sqlx::query!(
            r#"
            UPDATE pools
               SET last_seen_at = GREATEST(LEAST(last_seen_at, NOW()), LEAST($2, NOW()))
             WHERE pool_address = $1
            "#,
            pool.pool_address.to_string(),
            pool.last_seen_at,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        if updated.rows_affected() > 0 {
            return Ok(());
        }
        sqlx::query!(
            r#"
            INSERT INTO pools
                (pool_address, protocol, token_a_mint, token_b_mint,
                 first_seen_at, last_seen_at)
            VALUES ($1, $2, $3, $4, LEAST($5, NOW()), LEAST($5, NOW()))
            ON CONFLICT DO NOTHING
            "#,
            pool.pool_address.to_string(),
            pool.protocol.as_str(),
            pool.token_a_mint.map(|m| m.to_string()),
            pool.token_b_mint.map(|m| m.to_string()),
            pool.last_seen_at,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    /// Same rule as [`Self::upsert`], collapsed: with `NOW()` as the incoming
    /// value, `GREATEST(LEAST(last_seen_at, NOW()), NOW())` is `NOW()`.
    async fn touch_last_seen(&self, pool_address: &Pubkey) -> RepositoryResult<()> {
        sqlx::query!(
            r#"UPDATE pools SET last_seen_at = NOW() WHERE pool_address = $1"#,
            pool_address.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn mark_needs_refresh(&self, pool_address: &Pubkey) -> RepositoryResult<()> {
        sqlx::query!(
            r#"UPDATE pools SET needs_refresh = TRUE WHERE pool_address = $1"#,
            pool_address.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    /// One statement, so the properties and the flag can never disagree: either
    /// the refreshed values and the lowered flag both commit, or neither does
    /// and the pool is proposed again next cycle.
    async fn set_registry_properties(
        &self,
        pool_address: &Pubkey,
        core: &PoolRegistryProperties,
    ) -> RepositoryResult<()> {
        let fee_bps = convert_decimal_to_bigdecimal(core.fee_bps, "fee_bps")?;
        sqlx::query!(
            r#"
            UPDATE pools
            SET token_a_mint  = $2,
                token_b_mint  = $3,
                fee_bps       = $4,
                needs_refresh = FALSE
            WHERE pool_address = $1
            "#,
            pool_address.to_string(),
            core.token_a_mint.to_string(),
            core.token_b_mint.to_string(),
            fee_bps,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }
}

#[async_trait]
impl PoolCatalog for PgPoolRepository {
    async fn counts(&self) -> RepositoryResult<PoolCounts> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) AS "observed!",
                COUNT(*) FILTER (
                    WHERE first_seen_at > NOW() - INTERVAL '24 hours'
                ) AS "discovered_24h!"
            FROM pools
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(PoolCounts {
            observed: row.observed,
            discovered_24h: row.discovered_24h,
        })
    }

    async fn counts_by_protocol(&self) -> RepositoryResult<Vec<ProtocolPools>> {
        let rows = sqlx::query!(
            r#"
            SELECT protocol, COUNT(*) AS "pools_seen!"
              FROM pools
             GROUP BY protocol
             ORDER BY protocol
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| {
                Ok(ProtocolPools {
                    protocol: parse_protocol(&row.protocol)?,
                    pools_seen: row.pools_seen,
                })
            })
            .collect()
    }

    async fn find_by_addresses(&self, pool_addresses: &[Pubkey]) -> RepositoryResult<Vec<Pool>> {
        let addresses: Vec<String> = pool_addresses.iter().map(|p| p.to_string()).collect();
        let rows = sqlx::query_as!(
            PoolRow,
            r#"
            SELECT pool_address, protocol, token_a_mint, token_b_mint,
                   fee_bps AS "fee_bps?: rust_decimal::Decimal",
                   first_seen_at, last_seen_at
            FROM pools
            WHERE pool_address = ANY($1::TEXT[])
            "#,
            &addresses
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(Pool::try_from).collect()
    }
}
