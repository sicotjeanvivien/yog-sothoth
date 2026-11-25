//! One statement per order. ⚠️ One statement each: the next reading replaces the rows, and must
//! not fall between them and the reading's instant.

use sqlx::{PgPool, postgres::types::PgInterval};
use yog_core::{RepositoryResult, domain::Protocol};

use super::cursor::CursorKeys;
use super::rows::VestingUnlockRow;
use crate::repositories::helper::map_sqlx_error;

/// The releases of the window, the soonest first.
pub(super) async fn by_time(
    pool: &PgPool,
    protocol: Protocol,
    span: PgInterval,
    keys: &CursorKeys,
    limit: i64,
) -> RepositoryResult<Vec<VestingUnlockRow>> {
    sqlx::query_as!(
        VestingUnlockRow,
        r#"
                WITH reading AS (
                    SELECT bucket, measured_at, vesting_covered_since FROM analytic_protocol_tvl
                     WHERE protocol = $1 AND locked_usd IS NOT NULL
                     ORDER BY bucket DESC LIMIT 1
                )
                SELECT r.measured_at AS "as_of?", r.vesting_covered_since AS "covered_since?",
                       u.pool_address AS "pool_address?", ma.symbol AS "token_a_symbol?",
                       mb.symbol AS "token_b_symbol?", u.position AS "position?",
                       u.unlocks_at AS "unlocks_at?", u.liquidity AS "liquidity?",
                       u.amount_usd AS "amount_usd?",
                       ROUND(u.amount_usd, 6) AS "amount_key?"
                  FROM reading r
                  LEFT JOIN LATERAL (
                      SELECT * FROM analytic_vesting_unlocks u
                       WHERE u.protocol = $1 AND u.bucket = r.bucket
                         AND u.unlocks_at > r.measured_at
                         AND u.unlocks_at <= r.measured_at + $2::INTERVAL
                         AND (NOT $3
                              OR (u.unlocks_at, u.pool_address, u.position) > ($4, $5, $6))
                       ORDER BY u.unlocks_at, u.pool_address, u.position
                       LIMIT $7
                  ) u ON true
                  LEFT JOIN pools p ON p.pool_address = u.pool_address
                  LEFT JOIN token_metadata ma ON ma.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata mb ON mb.mint = p.token_b_mint::TEXT
                 ORDER BY u.unlocks_at, u.pool_address, u.position
                "#,
        protocol.as_str(),
        span,
        keys.given,
        keys.unlocks_at,
        keys.pool_address,
        keys.position,
        limit + 1,
    )
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)
}

/// The releases of the window, the largest first; an unvalued one last.
pub(super) async fn by_amount(
    pool: &PgPool,
    protocol: Protocol,
    span: PgInterval,
    keys: &CursorKeys,
    limit: i64,
) -> RepositoryResult<Vec<VestingUnlockRow>> {
    sqlx::query_as!(
        VestingUnlockRow,
        r#"
                WITH reading AS (
                    SELECT bucket, measured_at, vesting_covered_since FROM analytic_protocol_tvl
                     WHERE protocol = $1 AND locked_usd IS NOT NULL
                     ORDER BY bucket DESC LIMIT 1
                )
                SELECT r.measured_at AS "as_of?", r.vesting_covered_since AS "covered_since?",
                       u.pool_address AS "pool_address?", ma.symbol AS "token_a_symbol?",
                       mb.symbol AS "token_b_symbol?", u.position AS "position?",
                       u.unlocks_at AS "unlocks_at?", u.liquidity AS "liquidity?",
                       u.amount_usd AS "amount_usd?",
                       ROUND(u.amount_usd, 6) AS "amount_key?"
                  FROM reading r
                  LEFT JOIN LATERAL (
                      SELECT * FROM analytic_vesting_unlocks u
                       WHERE u.protocol = $1 AND u.bucket = r.bucket
                         AND u.unlocks_at > r.measured_at
                         AND u.unlocks_at <= r.measured_at + $2::INTERVAL
                         AND (NOT $3
                              -- ⚠️ Rounded as the cursor holds it: a `Decimal` keeps 28 digits, the column more.
                              OR ($8::NUMERIC IS NOT NULL
                                  AND (ROUND(u.amount_usd, 6) < $8 OR u.amount_usd IS NULL
                                       OR (ROUND(u.amount_usd, 6) = $8
                                           AND (u.unlocks_at, u.pool_address, u.position)
                                               > ($4, $5, $6))))
                              OR ($8::NUMERIC IS NULL AND u.amount_usd IS NULL
                                  AND (u.unlocks_at, u.pool_address, u.position) > ($4, $5, $6)))
                       ORDER BY ROUND(u.amount_usd, 6) DESC NULLS LAST, u.unlocks_at, u.pool_address,
                                u.position
                       LIMIT $7
                  ) u ON true
                  LEFT JOIN pools p ON p.pool_address = u.pool_address
                  LEFT JOIN token_metadata ma ON ma.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata mb ON mb.mint = p.token_b_mint::TEXT
                 ORDER BY ROUND(u.amount_usd, 6) DESC NULLS LAST, u.unlocks_at, u.pool_address, u.position
                "#,
        protocol.as_str(),
        span,
        keys.given,
        keys.unlocks_at,
        keys.pool_address,
        keys.position,
        limit + 1,
        keys.amount_usd,
    )
    .fetch_all(pool)
    .await
    .map_err(map_sqlx_error)
}
