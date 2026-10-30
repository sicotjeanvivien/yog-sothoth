//! PostgreSQL implementation of [`TokenActivityFeed`]: yog-analytic's
//! aggregate by token, each token's identity joined in the same read.

mod rows;

use async_trait::async_trait;
use rows::TokenActivityRow;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{TokenActivityCursor, TokenActivityFeed, TokenActivityPage, TranchePeriod, Window},
};

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_decimal_to_bigdecimal, map_sqlx_error,
};

pub struct PgTokenActivityRepository {
    pool: PgPool,
}

impl PgTokenActivityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TokenActivityFeed for PgTokenActivityRepository {
    async fn list(
        &self,
        window: Window,
        cursor: Option<TokenActivityCursor>,
        limit: i64,
    ) -> RepositoryResult<Option<TokenActivityPage>> {
        let had_cursor = cursor.is_some();
        let (after_volume, after_mint) = match cursor {
            Some(c) => (
                Some(convert_decimal_to_bigdecimal(
                    c.volume_usd,
                    "cursor volume_usd",
                )?),
                Some(c.mint.to_string()),
            ),
            None => (None, None),
        };
        // ⚠️ One statement: a freeze rewrites every row, and must not fall between
        // the hour read and the rows of that hour.
        let rows = sqlx::query_as!(
            TokenActivityRow,
            r#"
            SELECT w.bucket AS "last_hour?",
                   t.mint AS "mint?", t.volume_usd AS "volume_usd?", t.pools AS "pools?",
                   t.pool_hours AS "pool_hours?", t.pool_hours_valued AS "pool_hours_valued?",
                   m.symbol AS "symbol?", m.name AS "name?",
                   m.decimals AS "decimals?", m.logo_uri AS "logo_uri?",
                   m.metadata_provider AS "metadata_provider?",
                   m.fetched_at AS "fetched_at?", m.last_refresh_at AS "last_refresh_at?"
              -- ⚠️ Every row shares one hour: its first by volume names it, through the index.
              FROM (SELECT (SELECT bucket FROM analytic_token_periods
                             WHERE period = $1
                             ORDER BY volume_usd DESC, mint LIMIT 1) AS bucket) w
              LEFT JOIN LATERAL (
                  SELECT * FROM analytic_token_periods t
                   WHERE t.period = $1 AND t.bucket = w.bucket
                     AND ($2::NUMERIC IS NULL
                          OR t.volume_usd < $2
                          OR (t.volume_usd = $2 AND t.mint > $3))
                   ORDER BY t.volume_usd DESC, t.mint
                   LIMIT $4
              ) t ON true
              LEFT JOIN token_metadata m ON m.mint = t.mint
             ORDER BY t.volume_usd DESC, t.mint
            "#,
            window.as_str(),
            after_volume,
            after_mint,
            limit + 1,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let Some(last_hour) = rows.first().and_then(|row| row.last_hour) else {
            return Ok(None);
        };
        let items = rows
            .into_iter()
            .filter_map(|row| row.into_activity().transpose())
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page =
            PageBuilder::new(items, limit, QueryMode::Forward, had_cursor).finalize(|token| {
                Cursor::TokenActivity(TokenActivityCursor {
                    last_hour,
                    volume_usd: token.volume_usd,
                    mint: token.mint,
                })
            });
        // ⚠️ Forward only: a previous cursor would be read forward from the first item.
        Ok(Some(TokenActivityPage {
            last_hour,
            page: Page {
                prev_cursor: None,
                ..page
            },
        }))
    }

    async fn list_in_tranches(
        &self,
        period: &TranchePeriod,
        cursor: Option<TokenActivityCursor>,
        limit: i64,
    ) -> RepositoryResult<TokenActivityPage> {
        let had_cursor = cursor.is_some();
        let (after_volume, after_mint) = match cursor {
            Some(c) => (
                Some(convert_decimal_to_bigdecimal(
                    c.volume_usd,
                    "cursor volume_usd",
                )?),
                Some(c.mint.to_string()),
            ),
            None => (None, None),
        };
        // ⚠️ The rule of yog-analytic's frozen rankings, at the tranche: each pool summed first,
        // its mints joined once, the volume rounded for the cursor's Decimal.
        let rows = sqlx::query_as!(
            TokenActivityRow,
            r#"
            WITH by_pool AS (
                SELECT f.protocol, f.pool_address,
                       SUM(f.volume_in_a_usd + f.volume_in_b_usd) FILTER (WHERE f.valued)
                           AS volume_usd,
                       COUNT(*) AS pool_hours, COUNT(*) FILTER (WHERE f.valued) AS pool_hours_valued
                  FROM analytic_pool_5min f
                 WHERE f.bucket >= $1 AND f.bucket < $2
                 GROUP BY f.protocol, f.pool_address
            ),
            ranked AS (
                SELECT m.mint, ROUND(COALESCE(SUM(b.volume_usd), 0), 6) AS volume_usd,
                       COUNT(*) AS pools, SUM(b.pool_hours)::BIGINT AS pool_hours,
                       SUM(b.pool_hours_valued)::BIGINT AS pool_hours_valued
                  FROM by_pool b
                  JOIN pools p ON p.pool_address = b.pool_address AND p.protocol = b.protocol
                 CROSS JOIN LATERAL (VALUES (p.token_a_mint), (p.token_b_mint)) AS m (mint)
                 WHERE m.mint IS NOT NULL
                 GROUP BY m.mint
            )
            SELECT $2::TIMESTAMPTZ AS "last_hour?",
                   t.mint AS "mint?", t.volume_usd AS "volume_usd?", t.pools AS "pools?",
                   t.pool_hours AS "pool_hours?", t.pool_hours_valued AS "pool_hours_valued?",
                   m.symbol AS "symbol?", m.name AS "name?",
                   m.decimals AS "decimals?", m.logo_uri AS "logo_uri?",
                   m.metadata_provider AS "metadata_provider?",
                   m.fetched_at AS "fetched_at?", m.last_refresh_at AS "last_refresh_at?"
              -- ⚠️ Ranked before it is named: joined first, a page costs every token of the hour.
              FROM (
                  SELECT t.*
                    FROM ranked t
                   WHERE $3::NUMERIC IS NULL
                      OR t.volume_usd < $3
                      OR (t.volume_usd = $3 AND t.mint > $4)
                   ORDER BY t.volume_usd DESC, t.mint
                   LIMIT $5
              ) t
              LEFT JOIN token_metadata m ON m.mint = t.mint
             ORDER BY t.volume_usd DESC, t.mint
            "#,
            period.from,
            period.as_of,
            after_volume,
            after_mint,
            limit + 1,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let last_hour = period.as_of;
        let items = rows
            .into_iter()
            .filter_map(|row| row.into_activity().transpose())
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page =
            PageBuilder::new(items, limit, QueryMode::Forward, had_cursor).finalize(|token| {
                Cursor::TokenActivity(TokenActivityCursor {
                    last_hour,
                    volume_usd: token.volume_usd,
                    mint: token.mint,
                })
            });
        Ok(TokenActivityPage {
            last_hour,
            page: Page {
                prev_cursor: None,
                ..page
            },
        })
    }
}
