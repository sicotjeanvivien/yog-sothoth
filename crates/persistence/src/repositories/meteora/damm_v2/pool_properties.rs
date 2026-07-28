//! DAMM v2 pool-properties satellite repository (migration 036).
//!
//! Holds the pool properties that only exist for cp-amm, kept out of the
//! cross-protocol `pools` registry so no protocol carries NULL columns for
//! another protocol's concepts.
//!
//! Two writers, and the split is visible in the SQL below:
//!
//! - the **indexer** writes the fee shape (`base_fee_kind`, `has_dynamic_fee`),
//!   decoded from the genesis `InitializePool` blob — [`set_fee_config`];
//! - **yog-context** writes the fee-split percents as part of
//!   `PoolAccountResolver::set_pool_account` (in `repositories/pool.rs`), which
//!   fills the neutral `pools` columns and this satellite in one transaction.
//!
//! Each upsert therefore touches only its own columns on conflict: neither
//! writer may clobber the other's, and either may land first.
//!
//! [`set_fee_config`]: MeteoraDammV2PoolPropertiesRepository::set_fee_config

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{MeteoraDammV2PoolProperties, MeteoraDammV2PoolPropertiesRepository},
};

use crate::repositories::helper::{convert_string_to_pubkey, map_sqlx_error};

pub struct PgMeteoraDammV2PoolPropertiesRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2PoolPropertiesRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Row shape of `meteora_damm_v2_pool_properties`.
struct PoolPropertiesRow {
    pool_address: String,
    protocol_fee_percent: Option<i16>,
    partner_fee_percent: Option<i16>,
    referral_fee_percent: Option<i16>,
    base_fee_kind: Option<String>,
    has_dynamic_fee: Option<bool>,
}

/// Convert a SMALLINT fee-split percent back to the domain `u8`. The column
/// only ever holds values written from a `u8` (0..=100), but guard the range
/// rather than silently truncate a corrupt row — surfaces as `Integrity`.
fn percent_to_u8(value: Option<i16>, field: &str) -> Result<Option<u8>, RepositoryError> {
    value
        .map(|v| {
            u8::try_from(v)
                .map_err(|_| RepositoryError::Integrity(format!("{field} out of u8 range: {v}")))
        })
        .transpose()
}

impl TryFrom<PoolPropertiesRow> for MeteoraDammV2PoolProperties {
    type Error = RepositoryError;

    fn try_from(row: PoolPropertiesRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2PoolProperties {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol_fee_percent: percent_to_u8(row.protocol_fee_percent, "protocol_fee_percent")?,
            partner_fee_percent: percent_to_u8(row.partner_fee_percent, "partner_fee_percent")?,
            referral_fee_percent: percent_to_u8(row.referral_fee_percent, "referral_fee_percent")?,
            base_fee_kind: row.base_fee_kind,
            has_dynamic_fee: row.has_dynamic_fee,
        })
    }
}

#[async_trait]
impl MeteoraDammV2PoolPropertiesRepository for PgMeteoraDammV2PoolPropertiesRepository {
    async fn set_fee_config(
        &self,
        pool_address: &Pubkey,
        base_fee_kind: &str,
        has_dynamic_fee: bool,
    ) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_pool_properties
                (pool_address, base_fee_kind, has_dynamic_fee)
            VALUES ($1, $2, $3)
            ON CONFLICT (pool_address) DO UPDATE
                SET base_fee_kind   = EXCLUDED.base_fee_kind,
                    has_dynamic_fee = EXCLUDED.has_dynamic_fee
            "#,
            pool_address.to_string(),
            base_fee_kind,
            has_dynamic_fee,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
    ) -> RepositoryResult<Option<MeteoraDammV2PoolProperties>> {
        let row = sqlx::query_as!(
            PoolPropertiesRow,
            r#"
            SELECT pool_address, protocol_fee_percent, partner_fee_percent,
                   referral_fee_percent, base_fee_kind, has_dynamic_fee
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

#[cfg(test)]
#[path = "pool_properties_tests.rs"]
mod tests;
