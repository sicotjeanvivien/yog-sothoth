//! Postgres implementation of [`PoolCurrentStateRepository`].
//!
//! Implementation notes:
//!
//! * Stale-write protection is enforced in SQL via a `WHERE` clause on the
//!   `ON CONFLICT DO UPDATE` branch — out-of-order events leave the existing
//!   row untouched without raising an error.
//! * `last_sqrt_price` / `last_swap_at` are preserved on liquidity events by
//!   `COALESCE(EXCLUDED.x, pool_current_state.x)`, and vice versa for
//!   `liquidity` / `last_liquidity_at` on swap events.
//! * `updated_at` is bumped to `NOW()` on every accepted write.
//!
//! u128 columns map to `NUMERIC(39, 0)`; conversions go through the shared
//! helpers in `repository_utils` to keep the error mapping consistent across
//! the crate.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{LastEventKind, PoolCurrentState, PoolCurrentStateRepository, PoolCurrentStateUpsert},
};

use crate::repository_utils::{
    convert_bigdecimal_to_u128, convert_u128_to_bigdecimal, map_sqlx_error,
};

/// sqlx-backed implementation of [`PoolCurrentStateRepository`].
pub struct PgPoolCurrentStateRepository {
    pool: PgPool,
}

impl PgPoolCurrentStateRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

// -------------------------------------------------------------------------
// Row -> domain mapping
// -------------------------------------------------------------------------

/// Raw row mirror — mirrors the SELECT column order below.
#[derive(sqlx::FromRow)]
struct PoolCurrentStateRow {
    pool_address: String,
    protocol: String,
    last_event_at: DateTime<Utc>,
    last_event_kind: String,
    last_signature: String,
    reserve_a: sqlx::types::BigDecimal,
    reserve_b: sqlx::types::BigDecimal,
    last_sqrt_price: Option<sqlx::types::BigDecimal>,
    last_swap_at: Option<DateTime<Utc>>,
    liquidity: Option<sqlx::types::BigDecimal>,
    last_liquidity_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<PoolCurrentStateRow> for PoolCurrentState {
    type Error = RepositoryError;

    fn try_from(row: PoolCurrentStateRow) -> Result<Self, Self::Error> {
        let last_event_kind = LastEventKind::from_wire(&row.last_event_kind).ok_or_else(|| {
            RepositoryError::Integrity(format!(
                "invalid last_event_kind in pool_current_state: {}",
                row.last_event_kind
            ))
        })?;

        Ok(PoolCurrentState {
            pool_address: row.pool_address,
            protocol: row.protocol,
            last_event_at: row.last_event_at,
            last_event_kind,
            last_signature: row.last_signature,
            reserve_a: convert_bigdecimal_to_u128(row.reserve_a, "reserve_a")?,
            reserve_b: convert_bigdecimal_to_u128(row.reserve_b, "reserve_b")?,
            last_sqrt_price: row
                .last_sqrt_price
                .map(|v| convert_bigdecimal_to_u128(v, "last_sqrt_price"))
                .transpose()?,
            last_swap_at: row.last_swap_at,
            liquidity: row
                .liquidity
                .map(|v| convert_bigdecimal_to_u128(v, "liquidity"))
                .transpose()?,
            last_liquidity_at: row.last_liquidity_at,
            updated_at: row.updated_at,
        })
    }
}

// -------------------------------------------------------------------------
// Trait impl
// -------------------------------------------------------------------------

#[async_trait]
impl PoolCurrentStateRepository for PgPoolCurrentStateRepository {
    /// Upsert with a stale-write guard.
    ///
    /// The `ON CONFLICT DO UPDATE ... WHERE` clause makes this a no-op when
    /// the incoming `event_at` is older or equal to the stored value, without
    /// raising. `xmax = 0` distinguishes INSERT from UPDATE in `RETURNING` —
    /// combined with `fetch_optional`:
    ///
    /// * `Some(_)` — INSERT or UPDATE accepted (`Ok(true)`)
    /// * `None`    — UPDATE guard didn't match → stale write (`Ok(false)`)
    async fn upsert(&self, upsert: &PoolCurrentStateUpsert) -> RepositoryResult<bool> {
        let reserve_a = convert_u128_to_bigdecimal(upsert.reserve_a, "reserve_a");
        let reserve_b = convert_u128_to_bigdecimal(upsert.reserve_b, "reserve_b");
        let sqrt_price = upsert
            .sqrt_price
            .map(|v| convert_u128_to_bigdecimal(v, "sqrt_price"));
        let liquidity = upsert
            .liquidity
            .map(|v| convert_u128_to_bigdecimal(v, "liquidity"));

        // `last_swap_at` is set only for swap events; `last_liquidity_at` only
        // for liquidity events. The COALESCE in the UPDATE branch keeps the
        // previous value for the field the current event doesn't touch.
        let last_swap_at = match upsert.event_kind {
            LastEventKind::Swap => Some(upsert.event_at),
            _ => None,
        };
        let last_liquidity_at = match upsert.event_kind {
            LastEventKind::LiquidityAdd | LastEventKind::LiquidityRemove => Some(upsert.event_at),
            _ => None,
        };

        let outcome = sqlx::query!(
            r#"
            INSERT INTO pool_current_state (
                pool_address, protocol,
                last_event_at, last_event_kind, last_signature,
                reserve_a, reserve_b,
                last_sqrt_price, last_swap_at,
                liquidity, last_liquidity_at,
                updated_at
            )
            VALUES (
                $1, $2,
                $3, $4, $5,
                $6, $7,
                $8, $9,
                $10, $11,
                NOW()
            )
            ON CONFLICT (pool_address) DO UPDATE SET
                protocol           = EXCLUDED.protocol,
                last_event_at      = EXCLUDED.last_event_at,
                last_event_kind    = EXCLUDED.last_event_kind,
                last_signature     = EXCLUDED.last_signature,
                reserve_a          = EXCLUDED.reserve_a,
                reserve_b          = EXCLUDED.reserve_b,
                last_sqrt_price    = COALESCE(EXCLUDED.last_sqrt_price,   pool_current_state.last_sqrt_price),
                last_swap_at       = COALESCE(EXCLUDED.last_swap_at,      pool_current_state.last_swap_at),
                liquidity          = COALESCE(EXCLUDED.liquidity,         pool_current_state.liquidity),
                last_liquidity_at  = COALESCE(EXCLUDED.last_liquidity_at, pool_current_state.last_liquidity_at),
                updated_at         = NOW()
            WHERE pool_current_state.last_event_at < EXCLUDED.last_event_at
            RETURNING (xmax = 0) AS "inserted!"
            "#,
            upsert.pool_address,
            upsert.protocol,
            upsert.event_at,
            upsert.event_kind.as_str(),
            upsert.signature,
            reserve_a,
            reserve_b,
            sqrt_price,
            last_swap_at,
            liquidity,
            last_liquidity_at,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(outcome.is_some())
    }

    async fn get_by_address(
        &self,
        pool_address: &str,
    ) -> RepositoryResult<Option<PoolCurrentState>> {
        let row = sqlx::query_as!(
            PoolCurrentStateRow,
            r#"
            SELECT
                pool_address,
                protocol,
                last_event_at,
                last_event_kind,
                last_signature,
                reserve_a,
                reserve_b,
                last_sqrt_price,
                last_swap_at,
                liquidity,
                last_liquidity_at,
                updated_at
            FROM pool_current_state
            WHERE pool_address = $1
            "#,
            pool_address,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(PoolCurrentState::try_from).transpose()
    }

    async fn list_most_recent(
        &self,
        limit: u32,
        before_last_event_at: Option<DateTime<Utc>>,
    ) -> RepositoryResult<Vec<PoolCurrentState>> {
        if limit == 0 {
            return Err(RepositoryError::Integrity(
                "limit must be greater than 0".to_string(),
            ));
        }

        // Cap defensively to keep the i64 cast safe for absurd inputs.
        let limit_i64 = i64::from(limit.min(1_000));

        let rows = sqlx::query_as!(
            PoolCurrentStateRow,
            r#"
            SELECT
                pool_address,
                protocol,
                last_event_at,
                last_event_kind,
                last_signature,
                reserve_a,
                reserve_b,
                last_sqrt_price,
                last_swap_at,
                liquidity,
                last_liquidity_at,
                updated_at
            FROM pool_current_state
            WHERE ($1::TIMESTAMPTZ IS NULL OR last_event_at < $1)
            ORDER BY last_event_at DESC, pool_address ASC
            LIMIT $2
            "#,
            before_last_event_at,
            limit_i64,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolCurrentState::try_from).collect()
    }
}
