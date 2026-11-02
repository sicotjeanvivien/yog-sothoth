//! Postgres implementation of `ObservedSlotsRepository`, over `observed_slot_ranges`.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{ObservedSlots, ObservedSlotsRepository},
};

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};

#[derive(Clone)]
pub struct PgObservedSlotsRepository {
    pool: PgPool,
}

impl PgObservedSlotsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ObservedSlotsRepository for PgObservedSlotsRepository {
    async fn record(&self, observed: &ObservedSlots) -> RepositoryResult<()> {
        let first_slot = convert_u64_to_i64(observed.first_slot, "first_slot")?;
        let last_slot = convert_u64_to_i64(observed.last_slot, "last_slot")?;

        // ⚠️ Each end moves only outwards: a late write from a stale reading must not hand back
        // slots a later one recorded as watched.
        sqlx::query!(
            r#"
            INSERT INTO observed_slot_ranges AS r
                (protocol, session_started_at, first_slot, last_slot,
                 first_slot_at, last_slot_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, now())
            ON CONFLICT (protocol, session_started_at) DO UPDATE
            SET first_slot    = LEAST(r.first_slot, EXCLUDED.first_slot),
                first_slot_at = CASE WHEN EXCLUDED.first_slot < r.first_slot
                                     THEN EXCLUDED.first_slot_at ELSE r.first_slot_at END,
                last_slot     = GREATEST(r.last_slot, EXCLUDED.last_slot),
                last_slot_at  = CASE WHEN EXCLUDED.last_slot > r.last_slot
                                     THEN EXCLUDED.last_slot_at ELSE r.last_slot_at END,
                updated_at    = now()
            "#,
            observed.protocol.as_str(),
            observed.session_started_at,
            first_slot,
            last_slot,
            observed.first_slot_at,
            observed.last_slot_at,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
