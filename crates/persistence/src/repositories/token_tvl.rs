//! PostgreSQL implementation of [`TokenTvlLookup`]: yog-analytic's TVL by token, each token's
//! identity and the protocol's reading joined in the same read.

mod rows;

use async_trait::async_trait;
use rows::TokenTvlRow;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        Protocol, TokenTvlCursor, TokenTvlLookup, TokenTvlOrder, TokenTvlPage, TokenTvlTotal,
    },
};

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_bigdecimal_to_decimal, convert_decimal_to_bigdecimal,
    convert_optional, map_sqlx_error,
};

pub struct PgTokenTvlRepository {
    pool: PgPool,
}

impl PgTokenTvlRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TokenTvlLookup for PgTokenTvlRepository {
    async fn list(
        &self,
        protocol: Protocol,
        order: TokenTvlOrder,
        cursor: Option<TokenTvlCursor>,
        limit: i64,
    ) -> RepositoryResult<Option<TokenTvlPage>> {
        let had_cursor = cursor.is_some();
        let (after_value, after_mint) = match &cursor {
            Some(c) => (
                c.value
                    .map(|v| convert_decimal_to_bigdecimal(v, "cursor value"))
                    .transpose()?,
                Some(c.mint.to_string()),
            ),
            None => (None, None),
        };
        // ⚠️ One statement: the next reading replaces the rows, and must not fall between them and
        // the reading's instant.
        let rows = match order {
            TokenTvlOrder::Tvl => {
                sqlx::query_as!(
                    TokenTvlRow,
                    r#"
                WITH reading AS (
                    SELECT max(bucket) AS bucket FROM analytic_tvl_by_token WHERE protocol = $1
                )
                SELECT r.bucket + INTERVAL '1 hour' AS "at?",
                       p.tvl_usd AS "total_tvl_usd?", p.locked_usd AS "total_locked_usd?",
                       p.vesting_covered_since AS "covered_since?",
                       t.mint AS "mint?", t.tvl_usd AS "tvl_usd?", t.locked_usd AS "locked_usd?",
                       t.pools AS "pools?", ROUND(t.tvl_usd, 6) AS "value_key?",
                       m.symbol AS "symbol?", m.name AS "name?", m.decimals AS "decimals?",
                       m.logo_uri AS "logo_uri?", m.metadata_provider AS "metadata_provider?",
                       m.fetched_at AS "fetched_at?", m.last_refresh_at AS "last_refresh_at?"
                  FROM reading r
                  LEFT JOIN analytic_protocol_tvl p ON p.protocol = $1 AND p.bucket = r.bucket
                  LEFT JOIN LATERAL (
                      SELECT * FROM analytic_tvl_by_token t
                       WHERE t.protocol = $1
                         -- ⚠️ Rounded as the cursor holds it: a `Decimal` keeps 28 digits, the column more.
                         AND (NOT $4 OR ROUND(t.tvl_usd, 6) < $2
                              OR (ROUND(t.tvl_usd, 6) = $2 AND t.mint > $3))
                       ORDER BY ROUND(t.tvl_usd, 6) DESC, t.mint
                       LIMIT $5
                  ) t ON true
                  LEFT JOIN token_metadata m ON m.mint = t.mint
                 ORDER BY ROUND(t.tvl_usd, 6) DESC, t.mint
                "#,
                    protocol.as_str(),
                    after_value,
                    after_mint,
                    had_cursor,
                    limit + 1,
                )
                .fetch_all(&self.pool)
                .await
            }
            // An unknown locked part sorts last: the cursor's key is NULL there.
            TokenTvlOrder::Locked => {
                sqlx::query_as!(
                    TokenTvlRow,
                    r#"
                WITH reading AS (
                    SELECT max(bucket) AS bucket FROM analytic_tvl_by_token WHERE protocol = $1
                )
                SELECT r.bucket + INTERVAL '1 hour' AS "at?",
                       p.tvl_usd AS "total_tvl_usd?", p.locked_usd AS "total_locked_usd?",
                       p.vesting_covered_since AS "covered_since?",
                       t.mint AS "mint?", t.tvl_usd AS "tvl_usd?", t.locked_usd AS "locked_usd?",
                       t.pools AS "pools?", ROUND(t.locked_usd, 6) AS "value_key?",
                       m.symbol AS "symbol?", m.name AS "name?", m.decimals AS "decimals?",
                       m.logo_uri AS "logo_uri?", m.metadata_provider AS "metadata_provider?",
                       m.fetched_at AS "fetched_at?", m.last_refresh_at AS "last_refresh_at?"
                  FROM reading r
                  LEFT JOIN analytic_protocol_tvl p ON p.protocol = $1 AND p.bucket = r.bucket
                  LEFT JOIN LATERAL (
                      SELECT * FROM analytic_tvl_by_token t
                       WHERE t.protocol = $1
                         AND (NOT $4
                              OR ($2::NUMERIC IS NOT NULL
                                  AND (ROUND(t.locked_usd, 6) < $2
                                       OR (ROUND(t.locked_usd, 6) = $2 AND t.mint > $3)
                                       OR t.locked_usd IS NULL))
                              OR ($2::NUMERIC IS NULL AND t.locked_usd IS NULL AND t.mint > $3))
                       ORDER BY ROUND(t.locked_usd, 6) DESC NULLS LAST, t.mint
                       LIMIT $5
                  ) t ON true
                  LEFT JOIN token_metadata m ON m.mint = t.mint
                 ORDER BY ROUND(t.locked_usd, 6) DESC NULLS LAST, t.mint
                "#,
                    protocol.as_str(),
                    after_value,
                    after_mint,
                    had_cursor,
                    limit + 1,
                )
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(map_sqlx_error)?;

        let Some(first) = rows.first() else {
            return Ok(None);
        };
        let Some(at) = first.at else {
            return Ok(None);
        };
        let covered_since = first.covered_since;
        let total = TokenTvlTotal {
            tvl_usd: convert_optional(
                first.total_tvl_usd.clone(),
                "total_tvl_usd",
                convert_bigdecimal_to_decimal,
            )?,
            locked_usd: convert_optional(
                first.total_locked_usd.clone(),
                "total_locked_usd",
                convert_bigdecimal_to_decimal,
            )?,
        };
        let keyed = rows
            .into_iter()
            .filter_map(|row| row.into_token().transpose())
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page = PageBuilder::new(keyed, limit, QueryMode::Forward, had_cursor).finalize(
            |(token, key)| {
                Cursor::TokenTvl(Box::new(TokenTvlCursor {
                    protocol,
                    at,
                    order,
                    value: *key,
                    mint: token.mint,
                }))
            },
        );
        let page = Page {
            items: page.items.into_iter().map(|(token, _)| token).collect(),
            next_cursor: page.next_cursor,
            prev_cursor: page.prev_cursor,
            is_first: page.is_first,
            is_last: page.is_last,
        };
        // ⚠️ Forward only: a previous cursor would be read forward from the first item.
        Ok(Some(TokenTvlPage {
            at,
            covered_since,
            total,
            page: Page {
                prev_cursor: None,
                ..page
            },
        }))
    }
}
