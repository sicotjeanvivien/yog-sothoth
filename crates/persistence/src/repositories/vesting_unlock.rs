//! PostgreSQL implementation of [`VestingUnlockLookup`]: yog-analytic's vesting releases of its
//! last reading, each pool's token symbols joined in the same read.

mod rows;

use async_trait::async_trait;
use rows::VestingUnlockRow;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        Protocol, VestingUnlockCursor, VestingUnlockLookup, VestingUnlockOrder, VestingUnlockPage,
        Window,
    },
};

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_decimal_to_bigdecimal, interval_of, map_sqlx_error,
};

pub struct PgVestingUnlockRepository {
    pool: PgPool,
}

impl PgVestingUnlockRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl VestingUnlockLookup for PgVestingUnlockRepository {
    async fn list(
        &self,
        protocol: Protocol,
        window: Window,
        order: VestingUnlockOrder,
        cursor: Option<VestingUnlockCursor>,
        limit: i64,
    ) -> RepositoryResult<Option<VestingUnlockPage>> {
        let had_cursor = cursor.is_some();
        let (after_amount, after_at, after_pool, after_position) = match &cursor {
            Some(c) => (
                c.amount_usd
                    .map(|v| convert_decimal_to_bigdecimal(v, "cursor amount_usd"))
                    .transpose()?,
                Some(c.unlocks_at),
                Some(c.pool_address.to_string()),
                Some(c.position.to_string()),
            ),
            None => (None, None, None, None),
        };
        let span = interval_of(window.duration());
        // ⚠️ One statement: the next reading replaces the rows, and must not fall between them and
        // the reading's instant.
        let rows = match order {
            VestingUnlockOrder::Time => {
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
                       u.amount_usd AS "amount_usd?"
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
                    had_cursor,
                    after_at,
                    after_pool,
                    after_position,
                    limit + 1,
                )
                .fetch_all(&self.pool)
                .await
            }
            // An unvalued release sorts last: the cursor's amount is NULL there.
            VestingUnlockOrder::Amount => {
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
                       u.amount_usd AS "amount_usd?"
                  FROM reading r
                  LEFT JOIN LATERAL (
                      SELECT * FROM analytic_vesting_unlocks u
                       WHERE u.protocol = $1 AND u.bucket = r.bucket
                         AND u.unlocks_at > r.measured_at
                         AND u.unlocks_at <= r.measured_at + $2::INTERVAL
                         AND (NOT $3
                              OR ($8::NUMERIC IS NOT NULL
                                  AND (u.amount_usd < $8 OR u.amount_usd IS NULL
                                       OR (u.amount_usd = $8
                                           AND (u.unlocks_at, u.pool_address, u.position)
                                               > ($4, $5, $6))))
                              OR ($8::NUMERIC IS NULL AND u.amount_usd IS NULL
                                  AND (u.unlocks_at, u.pool_address, u.position) > ($4, $5, $6)))
                       ORDER BY u.amount_usd DESC NULLS LAST, u.unlocks_at, u.pool_address,
                                u.position
                       LIMIT $7
                  ) u ON true
                  LEFT JOIN pools p ON p.pool_address = u.pool_address
                  LEFT JOIN token_metadata ma ON ma.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata mb ON mb.mint = p.token_b_mint::TEXT
                 ORDER BY u.amount_usd DESC NULLS LAST, u.unlocks_at, u.pool_address, u.position
                "#,
                    protocol.as_str(),
                    span,
                    had_cursor,
                    after_at,
                    after_pool,
                    after_position,
                    limit + 1,
                    after_amount,
                )
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(map_sqlx_error)?;

        let Some(first) = rows.first() else {
            return Ok(None);
        };
        let Some(as_of) = first.as_of else {
            return Ok(None);
        };
        let covered_since = first.covered_since;
        let items = rows
            .into_iter()
            .filter_map(|row| row.into_unlock(protocol).transpose())
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page =
            PageBuilder::new(items, limit, QueryMode::Forward, had_cursor).finalize(|unlock| {
                Cursor::VestingUnlock(Box::new(VestingUnlockCursor {
                    protocol,
                    as_of,
                    window,
                    order,
                    amount_usd: unlock.amount_usd,
                    unlocks_at: unlock.unlocks_at,
                    pool_address: unlock.pool_address,
                    position: unlock.position,
                }))
            });
        // ⚠️ Forward only: a previous cursor would be read forward from the first item.
        Ok(Some(VestingUnlockPage {
            as_of,
            covered_since,
            page: Page {
                prev_cursor: None,
                ..page
            },
        }))
    }
}
