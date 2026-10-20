//! The DAMM v2 branch: one tranche of its swaps, its pools' prices written
//! and the swaps valued at them in a single statement.

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction, postgres::types::PgInterval};
use yog_core::{RepositoryResult, domain::Protocol};

use crate::repositories::helper::map_sqlx_error;

pub(super) async fn value_tranche(
    tx: &mut Transaction<'static, Postgres>,
    tranche: DateTime<Utc>,
    width: PgInterval,
) -> RepositoryResult<()> {
    sqlx::query_file!(
        "src/repositories/analytic/meteora_damm_v2_tranche.sql",
        tranche,
        Protocol::MeteoraDammV2.as_str(),
        width,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}
