//! DLMM pool-properties satellite (baseline §9): the Liquidity Book-only
//! properties, kept out of the cross-protocol `pools` registry, resolved by
//! yog-context through [`PoolAccountResolver`].
//! It writes this satellite and nothing else: the mints and `fee_bps` of the same
//! account go through [`yog_core::domain::PoolRepository::set_registry_properties`].

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{PoolAccountProperties, PoolAccountResolver, Protocol},
};

use crate::repositories::helper::{convert_string_to_pubkey, map_sqlx_error};

pub struct PgMeteoraDlmmPoolPropertiesRepository {
    pool: PgPool,
}

impl PgMeteoraDlmmPoolPropertiesRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolAccountResolver for PgMeteoraDlmmPoolPropertiesRepository {
    fn protocol(&self) -> Protocol {
        Protocol::MeteoraDlmm
    }

    /// Pools never resolved (a NULL column) or flagged `needs_refresh`, oldest first.
    /// `bin_step` stands for the satellite: its columns come from one read, NULL together
    /// or not at all. The flag is lowered by the registry write, which the caller issues last.
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        let rows = sqlx::query!(
            r#"
            SELECT p.pool_address
            FROM pools p
            LEFT JOIN meteora_dlmm_pool_properties props
                   ON props.pool_address = p.pool_address
            WHERE p.protocol = $2
              AND (p.needs_refresh
                OR p.token_a_mint IS NULL OR p.token_b_mint IS NULL OR p.fee_bps IS NULL
                OR props.pool_address IS NULL
                OR props.bin_step     IS NULL)
            ORDER BY p.first_seen_at
            LIMIT $1
            "#,
            limit,
            // ⚠️ Required: a pool of another protocol has no satellite row, forever.
            Protocol::MeteoraDlmm.as_str(),
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
    /// refused here, and a DLMM row on another protocol's pool by the composite foreign key.
    async fn set_pool_account(
        &self,
        pool_address: &Pubkey,
        properties: &PoolAccountProperties,
    ) -> RepositoryResult<()> {
        let PoolAccountProperties::MeteoraDlmm(properties) = properties else {
            return Err(yog_core::RepositoryError::Integrity(format!(
                "expected DLMM pool properties, got {:?}",
                properties.protocol()
            )));
        };

        sqlx::query!(
            r#"
            INSERT INTO meteora_dlmm_pool_properties
                (pool_address, bin_step, base_factor, base_fee_power_factor,
                 variable_fee_control, max_volatility_accumulator, protocol_share)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (pool_address) DO UPDATE
                SET bin_step                   = EXCLUDED.bin_step,
                    base_factor                = EXCLUDED.base_factor,
                    base_fee_power_factor      = EXCLUDED.base_fee_power_factor,
                    variable_fee_control       = EXCLUDED.variable_fee_control,
                    max_volatility_accumulator = EXCLUDED.max_volatility_accumulator,
                    protocol_share             = EXCLUDED.protocol_share
            "#,
            pool_address.to_string(),
            i32::from(properties.bin_step),
            i32::from(properties.base_factor),
            i16::from(properties.base_fee_power_factor),
            i64::from(properties.variable_fee_control),
            i64::from(properties.max_volatility_accumulator),
            i32::from(properties.protocol_share),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
