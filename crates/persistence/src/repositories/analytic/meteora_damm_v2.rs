//! The DAMM v2 branch: its swaps of the hours, summed from their 5-minute
//! tranches and valued as the activity view values them (`swap_parts`,
//! migration 010), one fact per (pool, hour).

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use yog_core::{RepositoryResult, domain::Protocol};

use crate::repositories::helper::map_sqlx_error;

/// Returns the facts upserted.
pub(super) async fn upsert_swap_facts(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
) -> RepositoryResult<u64> {
    let upserted = sqlx::query_file!(
        "src/repositories/analytic/meteora_damm_v2_swap_facts.sql",
        from,
        through,
        Protocol::MeteoraDammV2.as_str(),
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(upserted.rows_affected())
}
