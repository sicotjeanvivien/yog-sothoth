//! Postgres implementation of [`PoolCurrentStateRepository`].
//!
//! Implementation notes:
//!
//! * Ordering is enforced in SQL via a `WHERE` clause on the `ON CONFLICT DO
//!   UPDATE` branch, comparing `(slot, transaction_index, event_index)` as a
//!   tuple — out-of-order events leave the existing row untouched without
//!   raising an error. It used to compare `last_event_at`, whose second
//!   granularity rejected a third of all updates (baseline §4).
//! * `last_sqrt_price` / `last_swap_at` are preserved on liquidity events by
//!   `COALESCE(EXCLUDED.x, pool_current_state.x)`. That pair is the only
//!   kind-specific state left: a `liquidity` / `last_liquidity_at` pair was
//!   preserved symmetrically until migration 003 dropped it (it held a
//!   position's unsigned delta under a name claiming the pool's L).
//! * `updated_at` is bumped to `NOW()` on every accepted write.
//!
//! Column type mapping (matches the migration and the upstream event tables):
//!   * `reserve_a` / `reserve_b`                  BIGINT          ↔ u64
//!   * `last_sqrt_price`                          NUMERIC(39, 0)  ↔ u128
//!
//! Conversions go through the shared helpers in `repository_utils` to keep
//! error mapping consistent across the crate.
mod rows;

use crate::repositories::helper::{convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::PoolCurrentStateRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{
        LastEventKind, PoolCurrentState, PoolCurrentStateLookup, PoolCurrentStateRepository,
        PoolCurrentStateUpsert, PoolCurrentStateUpsertOutcome,
    },
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

#[async_trait]
impl PoolCurrentStateRepository for PgPoolCurrentStateRepository {
    /// Upsert guarded by the event's position in the chain.
    ///
    /// The `ON CONFLICT DO UPDATE … WHERE` clause makes this a no-op when the
    /// incoming `(slot, transaction_index, event_index)` is not strictly after
    /// the stored one, without raising.
    ///
    /// ## Why the statement is a CTE and not a bare INSERT
    ///
    /// A guarded `ON CONFLICT` returns **no row** when the guard fails, so the
    /// old form could only ever answer "applied or not". Telling apart a
    /// healthy rejection from the one case the key cannot rank — two
    /// transactions of the same slot, `transaction_index` being empty on this
    /// ingestion path — needs the state as it was *before* the write.
    ///
    /// `previous` supplies it in the same statement: a non-modifying CTE reads
    /// the snapshot taken before the statement ran, so it sees the pre-update
    /// row even though `upserted` overwrites it. One round-trip, and the guard
    /// itself stays atomic inside the `ON CONFLICT`.
    ///
    /// ## ⚠️ The ambiguity report is a lower bound under concurrency
    ///
    /// The guard and the report do **not** read the same row version. The
    /// `ON CONFLICT DO UPDATE … WHERE` clause is re-evaluated by Postgres
    /// against the latest committed version (EvalPlanQual); `previous` reads
    /// the statement snapshot. Under concurrent writers on one pool — the
    /// indexer runs up to `MAX_CONCURRENT_INDEX_TASKS` signatures at once — a
    /// task can be rejected by a row another task has just committed while its
    /// own snapshot never saw that slot, and it then reports
    /// `same_slot_ambiguity: false`.
    ///
    /// So the counter **undercounts, in the direction that flatters the
    /// assumption it exists to test**. Taking the previous row under
    /// `SELECT … FOR UPDATE` would close it, at the price of serialising every
    /// projection write per pool; for a metric, that trade is not worth it —
    /// but the bound must be stated rather than discovered later.
    async fn upsert(
        &self,
        upsert: &PoolCurrentStateUpsert,
    ) -> RepositoryResult<PoolCurrentStateUpsertOutcome> {
        let reserve_a = convert_u64_to_i64(upsert.reserve_a, "reserve_a")?;
        let reserve_b = convert_u64_to_i64(upsert.reserve_b, "reserve_b")?;
        let sqrt_price = upsert
            .sqrt_price
            .map(|v| convert_u128_to_bigdecimal(v, "sqrt_price"));

        // `last_swap_at` is set only for swap events. The COALESCE in the UPDATE
        // branch keeps the previous value when the current event is a liquidity
        // one, which doesn't touch it.
        let event_at = upsert.event_position.timestamp;
        let last_swap_at = match upsert.event_kind {
            LastEventKind::Swap => Some(event_at),
            _ => None,
        };

        let slot = convert_u64_to_i64(upsert.event_position.slot, "slot")?;
        let event_index = i32::from(upsert.event_position.event_index);
        let transaction_index = upsert.event_position.transaction_index.map(i64::from);
        let signature = upsert.event_position.signature.to_string();

        let row = sqlx::query!(
            r#"
            WITH previous AS (
                SELECT last_slot, last_signature
                FROM pool_current_state
                WHERE pool_address = $1
            ),
            upserted AS (
                INSERT INTO pool_current_state (
                    pool_address, protocol,
                    last_event_at, last_event_kind, last_signature,
                    reserve_a, reserve_b,
                    last_sqrt_price, last_swap_at,
                    updated_at,
                    last_slot, last_event_index, last_transaction_index
                )
                VALUES (
                    $1, $2,
                    $3, $4, $5,
                    $6, $7,
                    $8, $9,
                    NOW(),
                    $10, $11, $12
                )
                ON CONFLICT (pool_address) DO UPDATE SET
                    protocol               = EXCLUDED.protocol,
                    last_event_at          = EXCLUDED.last_event_at,
                    last_event_kind        = EXCLUDED.last_event_kind,
                    last_signature         = EXCLUDED.last_signature,
                    reserve_a              = EXCLUDED.reserve_a,
                    reserve_b              = EXCLUDED.reserve_b,
                    last_sqrt_price        = COALESCE(EXCLUDED.last_sqrt_price, pool_current_state.last_sqrt_price),
                    last_swap_at           = COALESCE(EXCLUDED.last_swap_at,    pool_current_state.last_swap_at),
                    updated_at             = NOW(),
                    last_slot              = EXCLUDED.last_slot,
                    last_event_index       = EXCLUDED.last_event_index,
                    last_transaction_index = EXCLUDED.last_transaction_index
                -- Lexicographic tuple comparison. `last_event_at` is still
                -- written above, for display, but no longer orders anything:
                -- it has second granularity and 56 % of swaps share theirs.
                -- The COALESCE lets gRPC make this a total order later by
                -- filling `transaction_index`, with no further migration.
                --
                -- Within one slot the comparison is NOT a fair tie-break, and
                -- the shortcut worth avoiding is calling it "wrong in either
                -- direction": `event_index` numbers the emissions of ONE
                -- transaction, so comparing it across two is comparing unlike
                -- things, and the state converges to whichever has the largest
                -- index. That systematically favours a leg deep inside a
                -- routed transaction over a single-leg swap of the same block.
                -- It is kept because it is **order-independent** — the final
                -- state is a function of the event set, not of delivery order,
                -- so a replay reproduces it. Last-writer-wins would be
                -- unbiased and non-deterministic instead.
                WHERE (
                        pool_current_state.last_slot,
                        COALESCE(pool_current_state.last_transaction_index, 0),
                        pool_current_state.last_event_index
                      ) < (
                        EXCLUDED.last_slot,
                        COALESCE(EXCLUDED.last_transaction_index, 0),
                        EXCLUDED.last_event_index
                      )
                RETURNING 1 AS applied
            )
            SELECT
                EXISTS (SELECT 1 FROM upserted)          AS "applied!",
                (SELECT last_slot      FROM previous)    AS previous_slot,
                (SELECT last_signature FROM previous)    AS previous_signature
            "#,
            upsert.pool_address.to_string(),
            &upsert.protocol.as_str(),
            event_at,
            upsert.event_kind.as_str(),
            signature,
            reserve_a,
            reserve_b,
            sqrt_price,
            last_swap_at,
            slot,
            event_index,
            transaction_index,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        // Same slot, different signature: two transactions of one block met on
        // this pool, and `(slot, _, event_index)` cannot say which came first.
        // Flagged whether or not the write applied — see the outcome's doc.
        let same_slot_ambiguity = row.previous_slot == Some(slot)
            && row.previous_signature.as_deref() != Some(signature.as_str());

        Ok(PoolCurrentStateUpsertOutcome {
            applied: row.applied,
            same_slot_ambiguity,
        })
    }
}

#[async_trait]
impl PoolCurrentStateLookup for PgPoolCurrentStateRepository {
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
