//! Postgres implementation of `TokenMetadataRepository`.
//!
//! Backed by the `token_metadata` table (migration 004).
//!
//! The domain types mints as `Pubkey`; the column is `TEXT`. The
//! conversion happens here, at the persistence boundary:
//! `Pubkey::to_string()` on write, `convert_string_to_pubkey` on
//! read.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::{
    RepositoryError, RepositoryResult,
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

        rows.into_iter()
            .map(|(mint,)| convert_string_to_pubkey(mint, "mint"))
            .collect()
    }

    async fn list_missing_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
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

    async fn find_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenMetadata>> {
        // Fetch the full row by mint. Returns None when no row
        // exists yet for that mint — not an error, the caller (the
        // token detail handler) decides what to do about it.
        let row = sqlx::query_as::<_, TokenMetadataRow>(
            r#"
            SELECT mint, symbol, name, decimals, logo_uri,
                   metadata_source, fetched_at, last_refresh_at
            FROM token_metadata
            WHERE mint = $1
            "#,
        )
        .bind(mint.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TokenMetadata::try_from).transpose()
    }
}

/// Row shape for reading `token_metadata`.
///
/// A thin sqlx-facing struct kept separate from the domain model so
/// the `mint` TEXT -> Pubkey conversion (fallible) can be expressed
/// via `TryFrom`, and the `decimals` SMALLINT -> u8 narrowing
/// stays out of the query function.
#[derive(sqlx::FromRow)]
struct TokenMetadataRow {
    mint: String,
    symbol: Option<String>,
    name: Option<String>,
    decimals: i16,
    logo_uri: Option<String>,
    metadata_source: String,
    fetched_at: DateTime<Utc>,
    last_refresh_at: DateTime<Utc>,
}

impl TryFrom<TokenMetadataRow> for TokenMetadata {
    type Error = RepositoryError;

    fn try_from(row: TokenMetadataRow) -> Result<Self, Self::Error> {
        // decimals is SMALLINT in Postgres (i16) but u8 in the
        // domain. A negative or out-of-range value would mean the
        // row was written with a non-conforming source.
        let decimals = u8::try_from(row.decimals).map_err(|_| {
            RepositoryError::Integrity(format!("invalid decimals: {}", row.decimals))
        })?;

        Ok(TokenMetadata {
            mint: convert_string_to_pubkey(row.mint, "mint")?,
            symbol: row.symbol,
            name: row.name,
            decimals,
            logo_uri: row.logo_uri,
            metadata_source: row.metadata_source,
            fetched_at: row.fetched_at,
            last_refresh_at: row.last_refresh_at,
        })
    }
}
