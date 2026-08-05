//! Postgres implementation of `TokenMetadataRepository`.
//!
//! Backed by the `token_metadata` table (baseline §6).
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
    domain::{TokenMetadata, TokenMetadataLookup, TokenMetadataRepository},
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
        // `IS NOT NULL` is load-bearing, and its absence used to break every
        // freshly bootstrapped database.
        //
        // `pools.token_a_mint` / `token_b_mint` are nullable — a pool is
        // discovered from the event stream before yog-context resolves its
        // mints from the on-chain account. One might expect
        // `NOT IN (SELECT mint FROM token_metadata)` to discard those NULLs on
        // its own, since `NULL NOT IN (…)` is NULL and a NULL predicate filters
        // the row out. It does — **unless the subquery is empty**, where SQL
        // defines `x NOT IN ()` as TRUE for any x, NULL included.
        //
        // So on a database whose `token_metadata` is still empty, an unresolved
        // pool sent a NULL through, the `!` below asserted non-null, and the
        // whole call failed with "unexpected null". The worker is skip-and-log,
        // so it warned and did nothing — and since it is itself what fills
        // `token_metadata`, nothing could break the cycle except the pool
        // account worker happening to resolve every pool first. Observed on a
        // fresh stack on 5 August 2026.
        //
        // `AS "mint!: String"` then keeps its meaning: with the NULLs excluded
        // by the WHERE, the column really is non-null, and sqlx's inference
        // through the UNION is too conservative to see it.
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
