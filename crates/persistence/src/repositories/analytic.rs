//! Postgres implementation of [`AnalyticRepository`]: one transaction per
//! cycle, whose only per-protocol part is each protocol's facts.
//!
//! [`AnalyticRepository`]: yog_core::domain::AnalyticRepository

mod meteora_damm_v2;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use yog_core::{
    RepositoryResult,
    domain::{AnalyticProgress, AnalyticRepository, CycleOutcome, CycleWrite},
};

#[derive(Clone)]
pub struct PgAnalyticRepository {
    pool: PgPool,
}

impl PgAnalyticRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnalyticRepository for PgAnalyticRepository {
    async fn progress(&self) -> RepositoryResult<AnalyticProgress> {
        let row = sqlx::query!(
            r#"
            SELECT MIN(bucket) AS "first_hour?: DateTime<Utc>",
                   MAX(bucket) FILTER (WHERE frozen_at IS NOT NULL)
                       AS "last_frozen?: DateTime<Utc>"
              FROM analytic_hours
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(AnalyticProgress {
            first_hour: row.first_hour,
            last_frozen: row.last_frozen,
        })
    }

    async fn write_cycle(&self, cycle: &CycleWrite) -> RepositoryResult<CycleOutcome> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        let cycle = cycle.without_frozen(last_frozen(&mut tx).await?);

        let pool_hours =
            meteora_damm_v2::upsert_swap_facts(&mut tx, cycle.from, cycle.through).await?;
        upsert_protocol_hourly(&mut tx, cycle.from, cycle.through).await?;
        record_hours(&mut tx, cycle.from, cycle.through).await?;
        let hours_frozen = match cycle.freeze_through {
            Some(through) => freeze(&mut tx, cycle.from, through, cycle.at).await?,
            None => 0,
        };
        if let Some(hour) = cycle.tvl_hour {
            measure_tvl(&mut tx, hour, cycle.at).await?;
        }

        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(CycleOutcome {
            pool_hours,
            hours_frozen,
        })
    }
}

/// Read inside the cycle's transaction: a freeze committed since the cycle
/// was planned must still keep its hours out of it.
async fn last_frozen(
    tx: &mut Transaction<'static, Postgres>,
) -> RepositoryResult<Option<DateTime<Utc>>> {
    sqlx::query_scalar!(
        r#"
        SELECT MAX(bucket) AS "last_frozen?: DateTime<Utc>"
          FROM analytic_hours
         WHERE frozen_at IS NOT NULL
        "#
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(map_sqlx_error)
}

/// Every protocol's facts of the hours, summed per protocol.
async fn upsert_protocol_hourly(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO analytic_protocol_hourly (
            protocol, bucket, volume_usd, fees_usd, lp_fees_usd,
            protocol_fees_usd, referral_fees_usd,
            swap_count, active_pools, active_pools_valued
        )
        SELECT protocol, bucket,
               SUM(volume_in_a_usd + volume_in_b_usd),
               SUM(fees_usd), SUM(lp_fees_usd),
               SUM(protocol_fees_usd), SUM(referral_fees_usd),
               SUM(swap_count), COUNT(*), COUNT(*) FILTER (WHERE valued)
          FROM analytic_pool_hourly
         WHERE bucket BETWEEN $1 AND $2
         GROUP BY protocol, bucket
        ON CONFLICT (protocol, bucket) DO UPDATE SET
            volume_usd          = EXCLUDED.volume_usd,
            fees_usd            = EXCLUDED.fees_usd,
            lp_fees_usd         = EXCLUDED.lp_fees_usd,
            protocol_fees_usd   = EXCLUDED.protocol_fees_usd,
            referral_fees_usd   = EXCLUDED.referral_fees_usd,
            swap_count          = EXCLUDED.swap_count,
            active_pools        = EXCLUDED.active_pools,
            active_pools_valued = EXCLUDED.active_pools_valued
        "#,
        from,
        through,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

async fn record_hours(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO analytic_hours (bucket)
        SELECT generate_series($1::TIMESTAMPTZ, $2::TIMESTAMPTZ, INTERVAL '1 hour')
        ON CONFLICT (bucket) DO NOTHING
        "#,
        from,
        through,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

async fn freeze(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
    at: DateTime<Utc>,
) -> RepositoryResult<u64> {
    let frozen = sqlx::query!(
        r#"
        UPDATE analytic_hours
           SET frozen_at = $3
         WHERE bucket BETWEEN $1 AND $2
           AND frozen_at IS NULL
        "#,
        from,
        through,
        at,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(frozen.rows_affected())
}

/// The current TVL of every protocol, by the rule of `pool_current_tvl`.
async fn measure_tvl(
    tx: &mut Transaction<'static, Postgres>,
    hour: DateTime<Utc>,
    at: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO analytic_protocol_tvl (protocol, bucket, tvl_usd, pools_priced, measured_at)
        SELECT p.protocol, $1, SUM(t.tvl_usd),
               COUNT(*) FILTER (WHERE t.tvl_usd IS NOT NULL), $2
          FROM pool_current_tvl t
          JOIN pools p ON p.pool_address = t.pool_address
         GROUP BY p.protocol
        ON CONFLICT (protocol, bucket) DO UPDATE SET
            tvl_usd      = EXCLUDED.tvl_usd,
            pools_priced = EXCLUDED.pools_priced,
            measured_at  = EXCLUDED.measured_at
        "#,
        hour,
        at,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}
