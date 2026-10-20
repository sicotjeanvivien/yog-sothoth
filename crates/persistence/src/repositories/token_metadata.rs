//! Postgres implementation of `TokenMetadataRepository`, over `token_metadata` (baseline §6).

pub(super) mod rows;

use crate::repositories::helper::{convert_string_to_pubkey, map_sqlx_error};
use async_trait::async_trait;
use chrono::Duration;
use rows::TokenMetadataRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{TokenMetadata, TokenMetadataLookup, TokenMetadataRepository},
};

#[derive(Clone)]
pub struct PgTokenMetadataRepository {
    pool: PgPool,
}

impl PgTokenMetadataRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TokenMetadataRepository for PgTokenMetadataRepository {
    async fn upsert(&self, metadata: &TokenMetadata) -> RepositoryResult<()> {
        let decimals = i16::from(metadata.decimals);

        sqlx::query!(
            r#"
            INSERT INTO token_metadata (
                mint, symbol, name, decimals, logo_uri,
                metadata_provider, fetched_at, last_refresh_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (mint) DO UPDATE
            SET symbol          = EXCLUDED.symbol,
                name            = EXCLUDED.name,
                decimals        = EXCLUDED.decimals,
                logo_uri        = EXCLUDED.logo_uri,
                metadata_provider = EXCLUDED.metadata_provider,
                last_refresh_at = EXCLUDED.last_refresh_at
            "#,
            metadata.mint.to_string(),
            metadata.symbol.as_deref(),
            metadata.name.as_deref(),
            decimals,
            metadata.logo_uri.as_deref(),
            metadata.metadata_provider.as_str(),
            metadata.fetched_at,
            metadata.last_refresh_at,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn list_price_candidates(&self, window: Duration) -> RepositoryResult<Vec<Pubkey>> {
        let mints = sqlx::query_scalar!(
            r#"
            SELECT mint FROM token_metadata
            WHERE mint IN (
                SELECT mint FROM token_last_price
                WHERE fetched_at >= (SELECT LEAST(max(fetched_at), now()) FROM token_last_price)
                                    - $1::BIGINT * INTERVAL '1 second'
                UNION ALL
                SELECT side.mint
                FROM pool_current_state pcs
                JOIN pools p ON p.pool_address = pcs.pool_address
                CROSS JOIN LATERAL (VALUES (p.token_a_mint), (p.token_b_mint)) AS side(mint)
                WHERE pcs.last_event_at >= now() - $1::BIGINT * INTERVAL '1 second'
            )
            "#,
            window.num_seconds(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        mints
            .into_iter()
            .map(|mint| convert_string_to_pubkey(mint, "mint"))
            .collect()
    }

    async fn list_missing_mints_of_recent_pools(
        &self,
        window: Duration,
    ) -> RepositoryResult<Vec<Pubkey>> {
        // ⚠️ `IS NOT NULL` is load-bearing: `NOT EXISTS` keeps a NULL mint, which fails the `!`.
        let mints = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT side.mint AS "mint!: String"
            FROM pools p
            CROSS JOIN LATERAL (VALUES (p.token_a_mint), (p.token_b_mint)) AS side(mint)
            WHERE p.first_seen_at >= now() - $1::BIGINT * INTERVAL '1 second'
              AND side.mint IS NOT NULL
              AND NOT EXISTS (SELECT 1 FROM token_metadata tm WHERE tm.mint = side.mint)
            "#,
            window.num_seconds(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        mints
            .into_iter()
            .map(|mint| convert_string_to_pubkey(mint, "mint"))
            .collect()
    }

    async fn list_missing_mints_of_all_pools(&self) -> RepositoryResult<Vec<Pubkey>> {
        // ⚠️ `IS NOT NULL` is load-bearing: `x NOT IN ()` is TRUE even for a NULL x, so on an empty
        // `token_metadata` an unresolved pool's NULL mint would fail the `!` and stall the worker.
        let mints = sqlx::query_scalar!(
            r#"
            SELECT mint AS "mint!: String" FROM (
                SELECT token_a_mint AS mint FROM pools
                UNION
                SELECT token_b_mint AS mint FROM pools
            ) AS all_mints
            WHERE mint IS NOT NULL
              AND mint NOT IN (SELECT mint FROM token_metadata)
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        mints
            .into_iter()
            .map(|mint| convert_string_to_pubkey(mint, "mint"))
            .collect()
    }
}

#[async_trait]
impl TokenMetadataLookup for PgTokenMetadataRepository {
    async fn find_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenMetadata>> {
        let row = sqlx::query_as!(
            TokenMetadataRow,
            r#"
            SELECT mint, symbol, name, decimals, logo_uri,
                   metadata_provider, fetched_at, last_refresh_at
            FROM token_metadata
            WHERE mint = $1
            "#,
            mint.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TokenMetadata::try_from).transpose()
    }
}
