//! The DAMM v2 branch: one tranche of its swaps, the prices of its pools'
//! tokens written first, then the swaps valued at them.

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use yog_core::{RepositoryResult, domain::Protocol};

use crate::repositories::helper::map_sqlx_error;

pub(super) async fn value_tranche(
    tx: &mut Transaction<'static, Postgres>,
    tranche: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query_file!(
        "src/repositories/analytic/meteora_damm_v2_pool_prices.sql",
        tranche,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    sqlx::query_file!(
        "src/repositories/analytic/meteora_damm_v2_swap_tranches.sql",
        tranche,
        Protocol::MeteoraDammV2.as_str(),
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}
