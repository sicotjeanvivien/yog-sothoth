//! One statement per order. ⚠️ One statement each: the next reading replaces the rows, and must
//! not fall between them and the reading's instant.

use sqlx::PgPool;
use yog_core::{RepositoryResult, domain::Protocol};

use super::cursor::CursorKeys;
use super::rows::TokenTvlRow;
use crate::repositories::helper::map_sqlx_error;

/// The tokens of the last reading, the most held first.
pub(super) async fn by_tvl(
    pool: &PgPool,
    protocol: Protocol,
    keys: &CursorKeys,
    limit: i64,
) -> RepositoryResult<Vec<TokenTvlRow>> {
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
        keys.value,
        keys.mint,
        keys.given,
        limit + 1,
    )
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)
}

/// The tokens of the last reading, the most locked first; an unknown locked part last.
pub(super) async fn by_locked(
    pool: &PgPool,
    protocol: Protocol,
    keys: &CursorKeys,
    limit: i64,
) -> RepositoryResult<Vec<TokenTvlRow>> {
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
        keys.value,
        keys.mint,
        keys.given,
        limit + 1,
    )
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)
}
