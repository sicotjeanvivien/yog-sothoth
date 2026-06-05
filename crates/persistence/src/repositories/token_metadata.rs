//! Postgres implementation of `TokenMetadataRepository`.
//!
//! Backed by the `token_metadata` table (migration 004).
//!
//! The domain types mints as `Pubkey`; the column is `TEXT`. The
//! conversion happens here, at the persistence boundary:
//! `Pubkey::to_string()` on write, `convert_string_to_pubkey` on
//! read.
mod rows;

use crate::repositories::helper::{convert_string_to_pubkey, map_sqlx_error};
use async_trait::async_trait;
use rows::TokenMetadataRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{TokenMetadata, TokenMetadataRepository},
};

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

    async fn list_known_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
        let mints = sqlx::query_scalar!("SELECT mint FROM token_metadata")
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        mints
            .into_iter()
            .map(|mint| convert_string_to_pubkey(mint, "mint"))
            .collect()
    }

    async fn list_missing_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
        // `AS "mint!: String"` forces sqlx to treat the column as
        // non-null String: nullability inference through the UNION
        // subquery is sometimes too conservative otherwise.
        let mints = sqlx::query_scalar!(
            r#"
            SELECT mint AS "mint!: String" FROM (
                SELECT token_a_mint AS mint FROM pools
                UNION
                SELECT token_b_mint AS mint FROM pools
            ) AS all_mints
            WHERE mint NOT IN (SELECT mint FROM token_metadata)
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
