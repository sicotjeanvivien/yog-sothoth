use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    domain::{PoolMetric, PoolMetricRepository},
    RepositoryResult,
};

use crate::infra::db::{
    convert_bigdecimal_to_u128, convert_i64_to_u64, convert_string_to_pubkey, convert_u64_to_i64,
    repository_utils::map_sqlx_error,
};

pub(crate) struct PgPoolMetricRepository {
    pool: PgPool,
}

impl PgPoolMetricRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolMetricRepository for PgPoolMetricRepository {
    async fn insert(&self, metric: &PoolMetric) -> RepositoryResult<()> {
        let price_q64 = sqlx::types::BigDecimal::from(metric.price_q64);

        sqlx::query!(
            r#"
            INSERT INTO pool_metrics
                (pool_address, signature,
                reserve_a, reserve_b, price_q64,
                price_impact_bps, imbalance_bps,
                current_fee_bps, fees_collected_a, fees_collected_b,
                volume_a, volume_b,
                active_bin_id, bin_step,
                timestamp)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
            metric.pool_address.to_string(),
            metric.signature,
            convert_u64_to_i64(metric.reserve_a, "reserve_a")?,
            convert_u64_to_i64(metric.reserve_b, "reserve_b")?,
            price_q64,
            metric.price_impact_bps,
            metric.imbalance_bps,
            metric.current_fee_bps,
            metric
                .fees_collected_a
                .map(|v| convert_u64_to_i64(v, "fees_collected_a"))
                .transpose()?,
            metric
                .fees_collected_b
                .map(|v| convert_u64_to_i64(v, "fees_collected_b"))
                .transpose()?,
            metric
                .volume_a
                .map(|v| convert_u64_to_i64(v, "volume_a"))
                .transpose()?,
            metric
                .volume_b
                .map(|v| convert_u64_to_i64(v, "volume_b"))
                .transpose()?,
            metric.active_bin_id,
            metric.bin_step,
            metric.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> RepositoryResult<Vec<PoolMetric>> {
        let rows = sqlx::query!(
            r#"
            SELECT pool_address, signature, reserve_a, reserve_b, price_q64, price_impact_bps, imbalance_bps, current_fee_bps, fees_collected_a, fees_collected_b, volume_a, volume_b, active_bin_id, bin_step, timestamp
            FROM pool_metrics
            WHERE pool_address = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
            pool_address.to_string(),
            limit
        ).fetch_all(&self.pool).await.map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| {
                Ok(PoolMetric {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    signature: row.signature,
                    reserve_a: convert_i64_to_u64(row.reserve_a, "reserve_a")?,
                    reserve_b: convert_i64_to_u64(row.reserve_b, "reserve_b")?,
                    price_q64: convert_bigdecimal_to_u128(row.price_q64, "price_q64")?,
                    price_impact_bps: row.price_impact_bps,
                    imbalance_bps: row.imbalance_bps,
                    current_fee_bps: row.current_fee_bps,
                    fees_collected_a: row
                        .fees_collected_a
                        .map(|v| convert_i64_to_u64(v, "fees_collected_a"))
                        .transpose()?,
                    fees_collected_b: row
                        .fees_collected_b
                        .map(|v| convert_i64_to_u64(v, "fees_collected_b"))
                        .transpose()?,
                    volume_a: row
                        .volume_a
                        .map(|v| convert_i64_to_u64(v, "volume_a"))
                        .transpose()?,
                    volume_b: row
                        .volume_b
                        .map(|v| convert_i64_to_u64(v, "volume_b"))
                        .transpose()?,
                    active_bin_id: row.active_bin_id,
                    bin_step: row.bin_step,
                    timestamp: row.timestamp,
                })
            })
            .collect::<RepositoryResult<Vec<_>>>()
    }
}
