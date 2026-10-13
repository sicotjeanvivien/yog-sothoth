//! The DAMM v2 branch: its swaps of the hours, valued as the activity view
//! values them (`swap_parts`, migration 010), one fact per (pool, hour).

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
    let upserted = sqlx::query!(
        r#"
        INSERT INTO analytic_pool_hourly (
            pool_address, bucket, protocol, swap_count, valued,
            volume_in_a_usd, volume_in_b_usd, fees_usd, lp_fees_usd,
            protocol_fees_usd, referral_fees_usd
        )
        SELECT pool_address, bucket, $3, swap_count, valued,
               volume_in_a_usd, volume_in_b_usd, fees_usd,
               fees_usd - protocol_fees_usd - referral_fees_usd,
               protocol_fees_usd, referral_fees_usd
          FROM (
            SELECT h.pool_address, h.bucket, h.swap_count,
                   h.valuation_complete AS valued,
                   CASE WHEN NOT h.valuation_complete THEN NULL
                        WHEN COALESCE(h.volume_in_a, 0) = 0 THEN 0
                        ELSE (h.volume_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a
                   END AS volume_in_a_usd,
                   CASE WHEN NOT h.valuation_complete THEN NULL
                        WHEN COALESCE(h.volume_in_b, 0) = 0 THEN 0
                        ELSE (h.volume_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b
                   END AS volume_in_b_usd,
                   CASE WHEN NOT h.valuation_complete THEN NULL ELSE
                       CASE WHEN COALESCE(h.fee_in_a, 0) = 0 THEN 0
                            ELSE (h.fee_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
                     + CASE WHEN COALESCE(h.fee_in_b, 0) = 0 THEN 0
                            ELSE (h.fee_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
                   END AS fees_usd,
                   CASE WHEN NOT h.valuation_complete THEN NULL ELSE
                       CASE WHEN COALESCE(h.protocol_fee_in_a, 0) = 0 THEN 0
                            ELSE (h.protocol_fee_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
                     + CASE WHEN COALESCE(h.protocol_fee_in_b, 0) = 0 THEN 0
                            ELSE (h.protocol_fee_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
                   END AS protocol_fees_usd,
                   CASE WHEN NOT h.valuation_complete THEN NULL ELSE
                       CASE WHEN COALESCE(h.referral_fee_in_a, 0) = 0 THEN 0
                            ELSE (h.referral_fee_in_a::NUMERIC / POWER(10::NUMERIC, h.dec_a)) * h.eff_price_a END
                     + CASE WHEN COALESCE(h.referral_fee_in_b, 0) = 0 THEN 0
                            ELSE (h.referral_fee_in_b::NUMERIC / POWER(10::NUMERIC, h.dec_b)) * h.eff_price_b END
                   END AS referral_fees_usd
              FROM meteora_damm_v2_swap_events_hourly_priced h
             WHERE h.bucket BETWEEN $1 AND $2
          ) valued
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
        Protocol::MeteoraDammV2.as_str(),
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(upserted.rows_affected())
}
