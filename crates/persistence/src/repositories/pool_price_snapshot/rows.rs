use std::str::FromStr;

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_bigdecimal_to_u128, convert_string_to_pubkey,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{PoolPriceSnapshot, Protocol},
};

/// Row shape for the snapshot query. Everything is non-null (the view's
/// INNER joins and `IS NOT NULL` predicates guarantee it, asserted by the
/// `!` overrides in the query), so no `Option` fields.
#[derive(sqlx::FromRow)]
pub(super) struct PoolPriceSnapshotRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) last_sqrt_price: BigDecimal,
    pub(super) last_swap_at: DateTime<Utc>,
    pub(super) decimals_a: i16,
    pub(super) decimals_b: i16,
    pub(super) price_a_usd: BigDecimal,
    pub(super) price_a_fetched_at: DateTime<Utc>,
    pub(super) price_b_usd: BigDecimal,
    pub(super) price_b_fetched_at: DateTime<Utc>,
}

/// Narrow a SMALLINT decimals column to the domain's u8.
fn convert_decimals(value: i16, field: &str) -> Result<u8, RepositoryError> {
    u8::try_from(value).map_err(|_| RepositoryError::Integrity(format!("invalid {field}: {value}")))
}

impl TryFrom<PoolPriceSnapshotRow> for PoolPriceSnapshot {
    type Error = RepositoryError;

    fn try_from(row: PoolPriceSnapshotRow) -> Result<Self, Self::Error> {
        Ok(PoolPriceSnapshot {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol: Protocol::from_str(&row.protocol)
                .map_err(|e| RepositoryError::Integrity(format!("invalid protocol: {e}")))?,
            sqrt_price: convert_bigdecimal_to_u128(row.last_sqrt_price, "last_sqrt_price")?,
            last_swap_at: row.last_swap_at,
            decimals_a: convert_decimals(row.decimals_a, "decimals_a")?,
            decimals_b: convert_decimals(row.decimals_b, "decimals_b")?,
            price_a_usd: convert_bigdecimal_to_decimal(row.price_a_usd, "price_a_usd")?,
            price_a_fetched_at: row.price_a_fetched_at,
            price_b_usd: convert_bigdecimal_to_decimal(row.price_b_usd, "price_b_usd")?,
            price_b_fetched_at: row.price_b_fetched_at,
        })
    }
}
