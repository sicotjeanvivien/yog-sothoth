mod log;
mod rows;

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_decimal_to_bigdecimal, convert_string_to_pubkey,
    map_sqlx_error, parse_protocol,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::{PoolEntryRow, PoolRow};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        Pool, PoolCatalog, PoolCursor, PoolEntry, PoolRegistryProperties, PoolRepository,
        PoolScope, ProtocolPools,
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

    async fn list_entries(
        &self,
        scope: PoolScope,
        cursor: Option<PoolCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<PoolEntry>> {
        let had_cursor = cursor.is_some();
        let (after_at, after_address) = match &cursor {
            Some(c) => (Some(c.first_seen_at), Some(c.pool_address.to_string())),
            None => (None, None),
        };
        let keys = self
            .list_keys(scope, after_at, after_address, limit + 1)
            .await?;
        let addresses: Vec<Pubkey> = keys
            .iter()
            .map(|key| convert_string_to_pubkey(key.clone(), "pool_address"))
            .collect::<RepositoryResult<_>>()?;
        let mut entries = self.read_entries(&addresses).await?;
        // ⚠️ The entries come back unordered: the page keeps the order of its keys.
        entries.sort_by_key(|entry| {
            addresses
                .iter()
                .position(|address| *address == entry.pool.pool_address)
        });
        let page =
            PageBuilder::new(entries, limit, QueryMode::Forward, had_cursor).finalize(|entry| {
                Cursor::Pool(Box::new(PoolCursor {
                    scope,
                    first_seen_at: entry.pool.first_seen_at,
                    pool_address: entry.pool.pool_address,
                }))
            });
        // ⚠️ Forward only: a previous cursor would be read forward from the first item.
        Ok(Page {
            prev_cursor: None,
            ..page
        })
    }

    async fn find_entry(&self, pool_address: &Pubkey) -> RepositoryResult<Option<PoolEntry>> {
        Ok(self.read_entries(&[*pool_address]).await?.pop())
    }
}

impl PgPoolRepository {
    /// The addresses of a page of `scope`'s pools after `(after_at, after_address)`, newest first.
    async fn list_keys(
        &self,
        scope: PoolScope,
        after_at: Option<DateTime<Utc>>,
        after_address: Option<String>,
        limit: i64,
    ) -> RepositoryResult<Vec<String>> {
        // ⚠️ The key compared column by column, not as a row: only this form bounds an index scan.
        let rows = match scope {
            PoolScope::All => {
                sqlx::query_scalar!(
                    r#"
                SELECT pool_address
                  FROM pools
                 WHERE first_seen_at <= COALESCE($1, 'infinity'::TIMESTAMPTZ)
                   AND (first_seen_at < COALESCE($1, 'infinity'::TIMESTAMPTZ)
                        OR pool_address < $2)
                 ORDER BY first_seen_at DESC, pool_address DESC
                 LIMIT $3
                "#,
                    after_at,
                    after_address,
                    limit,
                )
                .fetch_all(&self.pool)
                .await
            }
            PoolScope::Protocol(protocol) => {
                sqlx::query_scalar!(
                    r#"
                SELECT pool_address
                  FROM pools
                 WHERE protocol = $1
                   AND first_seen_at <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                   AND (first_seen_at < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                        OR pool_address < $3)
                 ORDER BY first_seen_at DESC, pool_address DESC
                 LIMIT $4
                "#,
                    protocol.as_str(),
                    after_at,
                    after_address,
                    limit,
                )
                .fetch_all(&self.pool)
                .await
            }
            PoolScope::Token(mint) => {
                sqlx::query_scalar!(
                    r#"
                SELECT pool_address
                  FROM pools
                 WHERE (token_a_mint = $1 OR token_b_mint = $1)
                   AND first_seen_at <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                   AND (first_seen_at < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                        OR pool_address < $3)
                 ORDER BY first_seen_at DESC, pool_address DESC
                 LIMIT $4
                "#,
                    mint.to_string(),
                    after_at,
                    after_address,
                    limit,
                )
                .fetch_all(&self.pool)
                .await
            }
        };
        rows.map_err(map_sqlx_error)
    }

    /// The entries of `addresses`, unordered, unknown addresses omitted.
    async fn read_entries(&self, addresses: &[Pubkey]) -> RepositoryResult<Vec<PoolEntry>> {
        let addresses: Vec<String> = addresses.iter().map(Pubkey::to_string).collect();
        let rows = sqlx::query_as!(
            PoolEntryRow,
            r#"
            SELECT p.pool_address, p.protocol, p.token_a_mint, p.token_b_mint,
                   p.fee_bps AS "fee_bps?: rust_decimal::Decimal",
                   p.first_seen_at, p.last_seen_at,
                   ta.symbol AS "a_symbol?", ta.name AS "a_name?", ta.decimals AS "a_decimals?",
                   ta.logo_uri AS "a_logo_uri?", ta.metadata_provider AS "a_metadata_provider?",
                   ta.fetched_at AS "a_fetched_at?", ta.last_refresh_at AS "a_last_refresh_at?",
                   tb.symbol AS "b_symbol?", tb.name AS "b_name?", tb.decimals AS "b_decimals?",
                   tb.logo_uri AS "b_logo_uri?", tb.metadata_provider AS "b_metadata_provider?",
                   tb.fetched_at AS "b_fetched_at?", tb.last_refresh_at AS "b_last_refresh_at?",
                   s.last_event_at AS "last_event_at?", s.last_event_kind AS "last_event_kind?",
                   s.reserve_a AS "reserve_a?", s.reserve_b AS "reserve_b?",
                   s.last_swap_at AS "last_swap_at?",
                   t.value_a_usd AS "value_a_usd?", t.value_b_usd AS "value_b_usd?",
                   t.tvl_usd AS "tvl_usd?"
              FROM pools p
              LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint
              LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint
              LEFT JOIN pool_current_state s ON s.pool_address = p.pool_address
              LEFT JOIN pool_current_tvl t ON t.pool_address = p.pool_address
             WHERE p.pool_address = ANY($1::TEXT[])
            "#,
            &addresses,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolEntry::try_from).collect()
    }
}
