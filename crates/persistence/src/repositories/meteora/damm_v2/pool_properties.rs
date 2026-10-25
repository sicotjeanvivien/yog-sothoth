//! DAMM v2 pool-properties satellite (baseline §8): the cp-amm-only properties,
//! kept out of the cross-protocol `pools` registry. Read by the api through
//! [`MeteoraDammV2PoolPropertiesLookup`], resolved by yog-context through [`PoolAccountResolver`].
//! It writes this satellite and nothing else: the registry columns of the same
//! account go through `PoolRepository`, without a transaction.

mod rows;

use async_trait::async_trait;
use rows::MeteoraDammV2PoolPropertiesRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        MeteoraDammV2PoolProperties, MeteoraDammV2PoolPropertiesLookup, PoolAccountProperties,
        PoolAccountResolver, Protocol,
    },
};

use crate::repositories::helper::{convert_string_to_pubkey, convert_u64_to_i64, map_sqlx_error};

pub struct PgMeteoraDammV2PoolPropertiesRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2PoolPropertiesRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2PoolPropertiesLookup for PgMeteoraDammV2PoolPropertiesRepository {
    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
    ) -> RepositoryResult<Option<MeteoraDammV2PoolProperties>> {
        let row = sqlx::query_as!(
            MeteoraDammV2PoolPropertiesRow,
            r#"
            SELECT pool_address, protocol_fee_percent,
                   referral_fee_percent, base_fee_kind, has_dynamic_fee,
                   cliff_fee_numerator, number_of_period, period_frequency,
                   reduction_factor, activation_point, activation_type
            FROM meteora_damm_v2_pool_properties
            WHERE pool_address = $1
            "#,
            pool_address.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(MeteoraDammV2PoolProperties::try_from).transpose()
    }
}

#[async_trait]
impl PoolAccountResolver for PgMeteoraDammV2PoolPropertiesRepository {
    fn protocol(&self) -> Protocol {
        Protocol::MeteoraDammV2
    }

    /// Pools never resolved (a NULL column) or flagged `needs_refresh`, oldest first.
    /// The flag is lowered by the registry write, which the caller issues last.
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        // ⚠️ Not `base_fee_kind`: NULL for a mode this build cannot map, it would requeue the
        // pool every cycle and starve the batch. `has_dynamic_fee` always decodes.
        let rows = sqlx::query!(
            r#"
            SELECT p.pool_address
            FROM pools p
            LEFT JOIN meteora_damm_v2_pool_properties props
                   ON props.pool_address = p.pool_address
            WHERE p.protocol = $2
              AND (p.needs_refresh
                OR p.token_a_mint IS NULL OR p.token_b_mint IS NULL OR p.fee_bps IS NULL
                OR props.pool_address           IS NULL
                OR props.protocol_fee_percent   IS NULL
                OR props.referral_fee_percent   IS NULL
                OR props.has_dynamic_fee        IS NULL)
            ORDER BY p.first_seen_at
            LIMIT $1
            "#,
            limit,
            // ⚠️ Required: a pool of another protocol has no satellite row, forever.
            Protocol::MeteoraDammV2.as_str(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|r| convert_string_to_pubkey(r.pool_address, "pool_address"))
            .collect()
    }

    /// Upserts this satellite only: its row is not created with the pool. Written before
    /// the registry, whose write lowers `needs_refresh`. A payload of another protocol is
    /// refused here, and a cp-amm row on another protocol's pool by the composite foreign key.
    async fn set_pool_account(
        &self,
        pool_address: &Pubkey,
        properties: &PoolAccountProperties,
    ) -> RepositoryResult<()> {
        let PoolAccountProperties::MeteoraDammV2(properties) = properties else {
            return Err(yog_core::RepositoryError::Integrity(format!(
                "expected cp-amm pool properties, got {:?}",
                properties.protocol()
            )));
        };

        let (protocol_pct, referral_pct) = (
            i16::from(properties.protocol_fee_percent),
            i16::from(properties.referral_fee_percent),
        );
        let base_fee_kind = properties.base_fee_kind.map(|kind| kind.as_str());

        // ⚠️ Dropped as a unit, not `?`: a curve that does not fit (cp-amm bounds `period_frequency`
        // by `!= 0` only) must not sink the whole write and keep the pool queued forever.
        let scheduler = properties.fee_scheduler.and_then(|s| {
            Some((
                convert_u64_to_i64(s.cliff_fee_numerator, "cliff_fee_numerator").ok()?,
                i32::from(s.number_of_period),
                convert_u64_to_i64(s.period_frequency, "period_frequency").ok()?,
                convert_u64_to_i64(s.reduction_factor, "reduction_factor").ok()?,
                convert_u64_to_i64(s.activation_point, "activation_point").ok()?,
                i16::from(s.activation_type),
            ))
        });
        let cliff = scheduler.map(|s| s.0);
        let number_of_period = scheduler.map(|s| s.1);
        let period_frequency = scheduler.map(|s| s.2);
        let reduction_factor = scheduler.map(|s| s.3);
        let activation_point = scheduler.map(|s| s.4);
        let activation_type = scheduler.map(|s| s.5);

        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_pool_properties
                (pool_address, protocol_fee_percent, referral_fee_percent,
                 base_fee_kind, has_dynamic_fee,
                 cliff_fee_numerator, number_of_period, period_frequency,
                 reduction_factor, activation_point, activation_type)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (pool_address) DO UPDATE
                SET protocol_fee_percent = EXCLUDED.protocol_fee_percent,
                    referral_fee_percent = EXCLUDED.referral_fee_percent,
                    has_dynamic_fee      = EXCLUDED.has_dynamic_fee,
                    -- COALESCE, not EXCLUDED: an account whose BaseFeeMode we
                    -- cannot map sends NULL, and that must not erase a kind an
                    -- earlier decode already established.
                    base_fee_kind        = COALESCE(EXCLUDED.base_fee_kind,
                                                    meteora_damm_v2_pool_properties.base_fee_kind),
                    -- Plain EXCLUDED here, deliberately unlike the line above.
                    -- These six describe a curve, and a NULL means "this read
                    -- established no curve" — which is the truth to record, not
                    -- a value to protect. Preserving them would let a pool whose
                    -- fee shape genuinely changed keep publishing a decay it no
                    -- longer has, and a stale curve is worse than none: it is
                    -- confidently wrong instead of visibly absent.
                    cliff_fee_numerator  = EXCLUDED.cliff_fee_numerator,
                    number_of_period     = EXCLUDED.number_of_period,
                    period_frequency     = EXCLUDED.period_frequency,
                    reduction_factor     = EXCLUDED.reduction_factor,
                    activation_point     = EXCLUDED.activation_point,
                    activation_type      = EXCLUDED.activation_type
            "#,
            pool_address.to_string(),
            protocol_pct,
            referral_pct,
            base_fee_kind,
            properties.has_dynamic_fee,
            cliff,
            number_of_period,
            period_frequency,
            reduction_factor,
            activation_point,
            activation_type,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
