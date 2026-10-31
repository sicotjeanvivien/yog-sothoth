//! Postgres implementation of `TokenMetadataRepository`, over `token_metadata` (baseline §6).

pub(super) mod rows;

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_decimal_to_bigdecimal, convert_string_to_pubkey, map_sqlx_error,
};
use async_trait::async_trait;
use chrono::Duration;
use rows::{TokenEntryRow, TokenMetadataRow};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        PriceCandidate, PriceCandidateClass, PriceCandidateParams, TokenCursor, TokenEntry,
        TokenMetadata, TokenMetadataLookup, TokenMetadataRepository,
    },
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

    async fn list_price_candidates(
        &self,
        params: &PriceCandidateParams,
    ) -> RepositoryResult<Vec<PriceCandidate>> {
        let low_value_below_usd =
            convert_decimal_to_bigdecimal(params.low_value_below_usd, "low_value_below_usd")?;
        let rows = sqlx::query_file!(
            "src/repositories/token_metadata/price_candidates.sql",
            params.window.num_seconds(),
            params.never_priced_grace.num_seconds(),
            low_value_below_usd,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| {
                Ok(PriceCandidate {
                    mint: convert_string_to_pubkey(row.mint, "mint")?,
                    class: PriceCandidateClass::of(
                        row.never_priced_past_grace,
                        row.below_low_value,
                    ),
                })
            })
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
        let mints = sqlx::query_file_scalar!(
            "src/repositories/token_metadata/missing_mints_of_all_pools.sql"
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

    async fn list_entries(
        &self,
        cursor: Option<TokenCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<TokenEntry>> {
        let had_cursor = cursor.is_some();
        let (after_at, after_mint) = match &cursor {
            Some(c) => (Some(c.fetched_at), Some(c.mint.to_string())),
            None => (None, None),
        };
        // ⚠️ `token_last_price` points at the latest row of `token_prices`, read by its key: the
        // `LIMIT 1` keeps the lookup a nested loop, which Postgres would turn into a full join.
        let rows = sqlx::query_as!(
            TokenEntryRow,
            r#"
            WITH page AS (
                SELECT mint, symbol, name, decimals, logo_uri, metadata_provider,
                       fetched_at, last_refresh_at
                  FROM token_metadata
                 WHERE fetched_at <= COALESCE($1, 'infinity'::TIMESTAMPTZ)
                   AND (fetched_at < COALESCE($1, 'infinity'::TIMESTAMPTZ) OR mint < $2)
                 ORDER BY fetched_at DESC, mint DESC
                 LIMIT $3
            )
            SELECT page.mint AS "mint!", page.symbol, page.name, page.decimals AS "decimals!",
                   page.logo_uri, page.metadata_provider AS "metadata_provider!",
                   page.fetched_at AS "fetched_at!", page.last_refresh_at AS "last_refresh_at!",
                   tp.price_usd AS "price_usd?: rust_decimal::Decimal",
                   tp.price_provider AS "price_provider?", tp.confidence AS "confidence?",
                   tp.fetched_at AS "price_fetched_at?"
              FROM page
              LEFT JOIN token_last_price l ON l.mint = page.mint
              LEFT JOIN LATERAL (
                  SELECT price_usd, price_provider, confidence, fetched_at
                    FROM token_prices
                   WHERE mint = l.mint AND fetched_at = l.fetched_at
                   LIMIT 1
              ) tp ON true
             ORDER BY page.fetched_at DESC, page.mint DESC
            "#,
            after_at,
            after_mint,
            limit + 1,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let entries = rows
            .into_iter()
            .map(TokenEntry::try_from)
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page =
            PageBuilder::new(entries, limit, QueryMode::Forward, had_cursor).finalize(|entry| {
                Cursor::Token(TokenCursor {
                    fetched_at: entry.metadata.fetched_at,
                    mint: entry.metadata.mint,
                })
            });
        // ⚠️ Forward only: a previous cursor would be read forward from the first item.
        Ok(Page {
            prev_cursor: None,
            ..page
        })
    }
}
