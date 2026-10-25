use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_i64_to_u64, convert_optional, convert_string_to_pubkey,
    parse_protocol,
};
use crate::repositories::token_metadata::rows::TokenMetadataRow;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{LastEventKind, Pool, PoolCurrentState, PoolEntry, PoolValue, TokenMetadata},
};

/// Row shape of `pools`. Per-protocol properties are read from their satellites,
/// such as [`PgMeteoraDammV2PoolPropertiesRepository`](crate::PgMeteoraDammV2PoolPropertiesRepository).
#[derive(sqlx::FromRow)]
pub(super) struct PoolRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) token_a_mint: Option<String>,
    pub(super) token_b_mint: Option<String>,
    pub(super) fee_bps: Option<Decimal>,
    pub(super) first_seen_at: DateTime<Utc>,
    pub(super) last_seen_at: DateTime<Utc>,
}

impl TryFrom<PoolRow> for Pool {
    type Error = RepositoryError;

    fn try_from(row: PoolRow) -> Result<Self, Self::Error> {
        Ok(Pool {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol: parse_protocol(&row.protocol)?,
            token_a_mint: convert_optional(
                row.token_a_mint,
                "token_a_mint",
                convert_string_to_pubkey,
            )?,
            token_b_mint: convert_optional(
                row.token_b_mint,
                "token_b_mint",
                convert_string_to_pubkey,
            )?,
            fee_bps: row.fee_bps,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
        })
    }
}

/// A pool, its tokens' metadata, its state and its value, each side's columns `None` when its
/// row is missing.
pub(super) struct PoolEntryRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) token_a_mint: Option<String>,
    pub(super) token_b_mint: Option<String>,
    pub(super) fee_bps: Option<Decimal>,
    pub(super) first_seen_at: DateTime<Utc>,
    pub(super) last_seen_at: DateTime<Utc>,
    pub(super) a_symbol: Option<String>,
    pub(super) a_name: Option<String>,
    pub(super) a_decimals: Option<i16>,
    pub(super) a_logo_uri: Option<String>,
    pub(super) a_metadata_provider: Option<String>,
    pub(super) a_fetched_at: Option<DateTime<Utc>>,
    pub(super) a_last_refresh_at: Option<DateTime<Utc>>,
    pub(super) b_symbol: Option<String>,
    pub(super) b_name: Option<String>,
    pub(super) b_decimals: Option<i16>,
    pub(super) b_logo_uri: Option<String>,
    pub(super) b_metadata_provider: Option<String>,
    pub(super) b_fetched_at: Option<DateTime<Utc>>,
    pub(super) b_last_refresh_at: Option<DateTime<Utc>>,
    pub(super) last_event_at: Option<DateTime<Utc>>,
    pub(super) last_event_kind: Option<String>,
    pub(super) reserve_a: Option<i64>,
    pub(super) reserve_b: Option<i64>,
    pub(super) last_swap_at: Option<DateTime<Utc>>,
    pub(super) value_a_usd: Option<BigDecimal>,
    pub(super) value_b_usd: Option<BigDecimal>,
    pub(super) tvl_usd: Option<BigDecimal>,
}

/// One side's metadata: present whole, or absent whole when its mint has no row yet.
struct SideColumns {
    mint: Option<String>,
    symbol: Option<String>,
    name: Option<String>,
    decimals: Option<i16>,
    logo_uri: Option<String>,
    metadata_provider: Option<String>,
    fetched_at: Option<DateTime<Utc>>,
    last_refresh_at: Option<DateTime<Utc>>,
}

fn convert_side(side: SideColumns) -> RepositoryResult<Option<TokenMetadata>> {
    let (
        Some(mint),
        Some(decimals),
        Some(metadata_provider),
        Some(fetched_at),
        Some(last_refresh_at),
    ) = (
        side.mint,
        side.decimals,
        side.metadata_provider,
        side.fetched_at,
        side.last_refresh_at,
    )
    else {
        return Ok(None);
    };
    TokenMetadata::try_from(TokenMetadataRow {
        mint,
        symbol: side.symbol,
        name: side.name,
        decimals,
        logo_uri: side.logo_uri,
        metadata_provider,
        fetched_at,
        last_refresh_at,
    })
    .map(Some)
}

/// ⚠️ Unvalued, not an error: one pool past a `Decimal` would fail its page and, without a
/// cursor, every page after it.
fn convert_value(value: Option<BigDecimal>, field: &str, pool_address: &str) -> Option<Decimal> {
    let value = value?;
    let shown = value.to_plain_string();
    convert_bigdecimal_to_decimal(value, field)
        .inspect_err(|_| super::log::value_out_of_range(pool_address, field, &shown))
        .ok()
}

fn parse_last_event_kind(raw: &str) -> RepositoryResult<LastEventKind> {
    [
        LastEventKind::Swap,
        LastEventKind::LiquidityAdd,
        LastEventKind::LiquidityRemove,
    ]
    .into_iter()
    .find(|kind| kind.as_str() == raw)
    .ok_or_else(|| RepositoryError::Integrity(format!("invalid last_event_kind: {raw}")))
}

impl TryFrom<PoolEntryRow> for PoolEntry {
    type Error = RepositoryError;

    fn try_from(row: PoolEntryRow) -> Result<Self, Self::Error> {
        let token_a = convert_side(SideColumns {
            mint: row.token_a_mint.clone(),
            symbol: row.a_symbol,
            name: row.a_name,
            decimals: row.a_decimals,
            logo_uri: row.a_logo_uri,
            metadata_provider: row.a_metadata_provider,
            fetched_at: row.a_fetched_at,
            last_refresh_at: row.a_last_refresh_at,
        })?;
        let token_b = convert_side(SideColumns {
            mint: row.token_b_mint.clone(),
            symbol: row.b_symbol,
            name: row.b_name,
            decimals: row.b_decimals,
            logo_uri: row.b_logo_uri,
            metadata_provider: row.b_metadata_provider,
            fetched_at: row.b_fetched_at,
            last_refresh_at: row.b_last_refresh_at,
        })?;
        let state = match (
            row.last_event_at,
            row.last_event_kind,
            row.reserve_a,
            row.reserve_b,
        ) {
            (Some(last_event_at), Some(kind), Some(reserve_a), Some(reserve_b)) => {
                Some(PoolCurrentState {
                    last_event_at,
                    last_event_kind: parse_last_event_kind(&kind)?,
                    reserve_a: convert_i64_to_u64(reserve_a, "reserve_a")?,
                    reserve_b: convert_i64_to_u64(reserve_b, "reserve_b")?,
                    last_swap_at: row.last_swap_at,
                })
            }
            _ => None,
        };
        let address = row.pool_address.clone();
        let pool = Pool::try_from(PoolRow {
            pool_address: row.pool_address,
            protocol: row.protocol,
            token_a_mint: row.token_a_mint,
            token_b_mint: row.token_b_mint,
            fee_bps: row.fee_bps,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
        })?;
        Ok(PoolEntry {
            pool,
            token_a,
            token_b,
            state,
            value: PoolValue {
                value_a_usd: convert_value(row.value_a_usd, "value_a_usd", &address),
                value_b_usd: convert_value(row.value_b_usd, "value_b_usd", &address),
                tvl_usd: convert_value(row.tvl_usd, "tvl_usd", &address),
            },
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
