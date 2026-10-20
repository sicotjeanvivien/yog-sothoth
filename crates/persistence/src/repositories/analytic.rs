//! Postgres implementation of [`AnalyticRepository`]: one transaction per
//! cycle, whose only per-protocol part is each protocol's tranches.
//!
//! [`AnalyticRepository`]: yog_core::domain::AnalyticRepository

mod meteora_damm_v2;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, PgPool, Postgres, Transaction};
use yog_core::{
    RepositoryResult,
    domain::{AnalyticProgress, AnalyticRepository, CycleOutcome, CycleWrite, TRANCHE},
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
        progress(&self.pool).await
    }

    async fn write_cycle(&self, cycle: &CycleWrite) -> RepositoryResult<CycleOutcome> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        lock_out_other_writers(&mut tx).await?;
        let cycle = cycle.without_written(progress(&mut *tx).await?);

        let mut tranches_written = 0;
        let mut tranche = cycle.tranches_from;
        // ⚠️ One statement per tranche: a whole catch-up in one would outlast its timeout.
        while tranche <= cycle.tranches_through {
            meteora_damm_v2::value_tranche(&mut tx, tranche).await?;
            record_tranche(&mut tx, tranche, cycle.at).await?;
            tranches_written += 1;
            tranche += TRANCHE;
        }
        let pool_hours = upsert_pool_hourly(&mut tx, cycle.from, cycle.through).await?;
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
            written: cycle,
            tranches_written,
            pool_hours,
            hours_frozen,
        })
    }
}

/// ⚠️ Before the progress is read: a concurrent cycle is then either committed
/// and seen, or not begun. Readers are not blocked.
async fn lock_out_other_writers(tx: &mut Transaction<'static, Postgres>) -> RepositoryResult<()> {
    sqlx::query!("LOCK TABLE analytic_hours IN EXCLUSIVE MODE")
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx_error)?;
    Ok(())
}

/// Read again inside the cycle's transaction: what a cycle committed since
/// this one was planned must stay out of it.
async fn progress<'e>(executor: impl PgExecutor<'e>) -> RepositoryResult<AnalyticProgress> {
    let row = sqlx::query!(
        r#"
        SELECT (SELECT MIN(bucket) FROM analytic_hours) AS "first_hour?: DateTime<Utc>",
               (SELECT MAX(bucket) FROM analytic_hours WHERE frozen_at IS NOT NULL)
                   AS "last_frozen?: DateTime<Utc>",
               (SELECT MAX(bucket) FROM analytic_tranches) AS "last_tranche?: DateTime<Utc>"
        "#
    )
    .fetch_one(executor)
    .await
    .map_err(map_sqlx_error)?;

    Ok(AnalyticProgress {
        first_hour: row.first_hour,
        last_frozen: row.last_frozen,
        last_tranche: row.last_tranche,
    })
}

/// ⚠️ A plain insert: a tranche written twice is a bug, and fails the cycle.
async fn record_tranche(
    tx: &mut Transaction<'static, Postgres>,
    tranche: DateTime<Utc>,
    at: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!(
        "INSERT INTO analytic_tranches (bucket, written_at) VALUES ($1, $2)",
        tranche,
        at,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

/// Every protocol's valued tranches of the hours, summed per pool and hour.
/// Returns the facts upserted.
async fn upsert_pool_hourly(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
) -> RepositoryResult<u64> {
    let upserted = sqlx::query!(
        r#"
        INSERT INTO analytic_pool_hourly (
            pool_address, bucket, protocol, swap_count, valued,
            volume_in_a_usd, volume_in_b_usd, fees_usd, lp_fees_usd,
            protocol_fees_usd, referral_fees_usd
        )
        SELECT pool_address, time_bucket('1 hour', bucket), protocol,
               SUM(swap_count), bool_and(valued),
               CASE WHEN bool_and(valued) THEN SUM(volume_in_a_usd) END,
               CASE WHEN bool_and(valued) THEN SUM(volume_in_b_usd) END,
               CASE WHEN bool_and(valued) THEN SUM(fees_usd) END,
               CASE WHEN bool_and(valued) THEN SUM(lp_fees_usd) END,
               CASE WHEN bool_and(valued) THEN SUM(protocol_fees_usd) END,
               CASE WHEN bool_and(valued) THEN SUM(referral_fees_usd) END
          FROM analytic_pool_5min
         WHERE bucket >= $1 AND bucket < $2::TIMESTAMPTZ + INTERVAL '1 hour'
         GROUP BY pool_address, time_bucket('1 hour', bucket), protocol
        ON CONFLICT (pool_address, bucket) DO UPDATE SET
            protocol          = EXCLUDED.protocol,
            swap_count        = EXCLUDED.swap_count,
            valued            = EXCLUDED.valued,
            volume_in_a_usd   = EXCLUDED.volume_in_a_usd,
            volume_in_b_usd   = EXCLUDED.volume_in_b_usd,
            fees_usd          = EXCLUDED.fees_usd,
            lp_fees_usd       = EXCLUDED.lp_fees_usd,
            protocol_fees_usd = EXCLUDED.protocol_fees_usd,
            referral_fees_usd = EXCLUDED.referral_fees_usd
        "#,
        from,
        through,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(upserted.rows_affected())
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
