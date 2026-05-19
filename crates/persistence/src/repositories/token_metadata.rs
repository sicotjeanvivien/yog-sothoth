//! Postgres implementation of `TokenMetadataRepository`.
//!
//! Backed by the `token_metadata` table (migration 004).
//!
//! The domain types mints as `Pubkey`; the column is `TEXT`. The
//! conversion happens here, at the persistence boundary:
//! `Pubkey::to_string()` on write, `convert_string_to_pubkey` on
//! read.

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::{
    RepositoryResult,
    domain::{TokenMetadata, TokenMetadataRepository},
};

use crate::repository_utils::{convert_string_to_pubkey, map_sqlx_error};

/// Postgres-backed token metadata repository.
#[derive(Clone)]
pub struct PgTokenMetadataRepository {
    pool: PgPool,
}

impl PgTokenMetadataRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TokenMetadataRepository for PgTokenMetadataRepository {
    async fn upsert(&self, metadata: &TokenMetadata) -> RepositoryResult<()> {
        // decimals is u8 in the domain, SMALLINT (i16) in Postgres —
        // the widening is always safe.
        let decimals = i16::from(metadata.decimals);

        sqlx::query(
            r#"
            INSERT INTO token_metadata (
                mint, symbol, name, decimals, logo_uri,
                metadata_source, fetched_at, last_refresh_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (mint) DO UPDATE
            SET symbol          = EXCLUDED.symbol,
                name            = EXCLUDED.name,
                decimals        = EXCLUDED.decimals,
                logo_uri        = EXCLUDED.logo_uri,
                metadata_source = EXCLUDED.metadata_source,
                last_refresh_at = EXCLUDED.last_refresh_at
            "#,
        )
        // Pubkey -> TEXT: base58 string.
        .bind(metadata.mint.to_string())
        .bind(&metadata.symbol)
        .bind(&metadata.name)
        .bind(decimals)
        .bind(&metadata.logo_uri)
        .bind(&metadata.metadata_source)
        .bind(metadata.fetched_at)
        .bind(metadata.last_refresh_at)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn list_known_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT mint FROM token_metadata")
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        // TEXT -> Pubkey for each row; a malformed value is a data
        // integrity error, surfaced by `convert_string_to_pubkey`.
        rows.into_iter()
            .map(|(mint,)| convert_string_to_pubkey(mint, "mint"))
            .collect()
    }

    async fn list_missing_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
        // Every distinct mint seen in `pools` (token A or token B)
        // that has no `token_metadata` row yet.
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT mint FROM (
                SELECT token_a_mint AS mint FROM pools
                UNION
                SELECT token_b_mint AS mint FROM pools
            ) AS all_mints
            WHERE mint NOT IN (SELECT mint FROM token_metadata)
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|(mint,)| convert_string_to_pubkey(mint, "mint"))
            .collect()
    }
}
