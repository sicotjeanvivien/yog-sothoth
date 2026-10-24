use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{
        DiscoveredTvl, LatestProtocolTvl, LiquidityFlows, LiquidityMove, LiquidityMoveKind,
        ProtocolActivity, ProtocolActivityBar, ProtocolTvl,
    },
};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_optional, parse_protocol, parse_window,
};

pub(super) struct ProtocolActivityRow {
    pub(super) protocol: String,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) fees_usd: Option<BigDecimal>,
    pub(super) lp_fees_usd: Option<BigDecimal>,
    pub(super) protocol_fees_usd: Option<BigDecimal>,
    pub(super) referral_fees_usd: Option<BigDecimal>,
    pub(super) swap_count: i64,
    pub(super) active_pools: Option<i64>,
    pub(super) births: i64,
    pub(super) pool_hours: i64,
    pub(super) pool_hours_valued: i64,
    pub(super) liquidity_added_usd: Option<BigDecimal>,
    pub(super) liquidity_removed_usd: Option<BigDecimal>,
    pub(super) liquidity_moves: Option<i64>,
    pub(super) liquidity_moves_valued: Option<i64>,
}

pub(super) struct ProtocolActivityBarRow {
    pub(super) protocol: String,
    pub(super) bar_from: DateTime<Utc>,
    pub(super) volume_usd: Option<BigDecimal>,
    pub(super) fees_usd: Option<BigDecimal>,
    pub(super) swap_count: i64,
    pub(super) pool_hours: i64,
    pub(super) pool_hours_valued: i64,
    pub(super) liquidity_added_usd: Option<BigDecimal>,
    pub(super) liquidity_removed_usd: Option<BigDecimal>,
    pub(super) liquidity_moves: Option<i64>,
    pub(super) liquidity_moves_valued: Option<i64>,
}

pub(super) struct ProtocolTvlRow {
    pub(super) protocol: String,
    pub(super) at: DateTime<Utc>,
    pub(super) tvl_usd: Option<BigDecimal>,
    pub(super) pools_priced: i64,
    pub(super) pools_unvalued: i64,
}

/// A protocol's last TVL row, once per discovered part measured with it; the
/// `discovered_*` columns are `None` together, on a TVL row without any.
pub(super) struct LatestProtocolTvlRow {
    pub(super) protocol: String,
    pub(super) at: DateTime<Utc>,
    pub(super) tvl_usd: Option<BigDecimal>,
    pub(super) pools_priced: i64,
    pub(super) pools_unvalued: i64,
    pub(super) discovered_period: Option<String>,
    pub(super) discovered_usd: Option<BigDecimal>,
    pub(super) discovered_pools: Option<i64>,
}

pub(super) struct LiquidityMoveRow {
    pub(super) protocol: String,
    pub(super) at: DateTime<Utc>,
    pub(super) signature: String,
    pub(super) pool_address: String,
    pub(super) token_a_symbol: Option<String>,
    pub(super) token_b_symbol: Option<String>,
    pub(super) owner: String,
    pub(super) kind: String,
    pub(super) amount_usd: BigDecimal,
}

fn usd(value: Option<BigDecimal>, field: &str) -> RepositoryResult<Option<rust_decimal::Decimal>> {
    convert_optional(value, field, convert_bigdecimal_to_decimal)
}

/// The liquidity columns read together: all set, or all `NULL` before it was measured.
fn liquidity(
    added: Option<BigDecimal>,
    removed: Option<BigDecimal>,
    moves: Option<i64>,
    moves_valued: Option<i64>,
) -> RepositoryResult<Option<LiquidityFlows>> {
    match (added, removed, moves, moves_valued) {
        (Some(added), Some(removed), Some(moves), Some(moves_valued)) => Ok(Some(LiquidityFlows {
            added_usd: convert_bigdecimal_to_decimal(added, "liquidity_added_usd")?,
            removed_usd: convert_bigdecimal_to_decimal(removed, "liquidity_removed_usd")?,
            moves,
            moves_valued,
        })),
        (None, None, None, None) => Ok(None),
        _ => Err(RepositoryError::Integrity(
            "liquidity read without all of its columns".to_owned(),
        )),
    }
}

impl TryFrom<ProtocolActivityRow> for ProtocolActivity {
    type Error = RepositoryError;

    fn try_from(row: ProtocolActivityRow) -> Result<Self, Self::Error> {
        Ok(ProtocolActivity {
            protocol: parse_protocol(&row.protocol)?,
            volume_usd: usd(row.volume_usd, "volume_usd")?,
            fees_usd: usd(row.fees_usd, "fees_usd")?,
            lp_fees_usd: usd(row.lp_fees_usd, "lp_fees_usd")?,
            protocol_fees_usd: usd(row.protocol_fees_usd, "protocol_fees_usd")?,
            referral_fees_usd: usd(row.referral_fees_usd, "referral_fees_usd")?,
            swap_count: row.swap_count,
            active_pools: row.active_pools,
            births: row.births,
            pool_hours: row.pool_hours,
            pool_hours_valued: row.pool_hours_valued,
            liquidity: liquidity(
                row.liquidity_added_usd,
                row.liquidity_removed_usd,
                row.liquidity_moves,
                row.liquidity_moves_valued,
            )?,
        })
    }
}

impl TryFrom<ProtocolActivityBarRow> for ProtocolActivityBar {
    type Error = RepositoryError;

    fn try_from(row: ProtocolActivityBarRow) -> Result<Self, Self::Error> {
        Ok(ProtocolActivityBar {
            protocol: parse_protocol(&row.protocol)?,
            from: row.bar_from,
            volume_usd: usd(row.volume_usd, "volume_usd")?,
            fees_usd: usd(row.fees_usd, "fees_usd")?,
            swap_count: row.swap_count,
            pool_hours: row.pool_hours,
            pool_hours_valued: row.pool_hours_valued,
            liquidity: liquidity(
                row.liquidity_added_usd,
                row.liquidity_removed_usd,
                row.liquidity_moves,
                row.liquidity_moves_valued,
            )?,
        })
    }
}

impl TryFrom<LiquidityMoveRow> for LiquidityMove {
    type Error = RepositoryError;

    fn try_from(row: LiquidityMoveRow) -> Result<Self, Self::Error> {
        Ok(LiquidityMove {
            protocol: parse_protocol(&row.protocol)?,
            at: row.at,
            signature: row.signature,
            pool_address: row.pool_address,
            token_a_symbol: row.token_a_symbol,
            token_b_symbol: row.token_b_symbol,
            owner: row.owner,
            kind: match row.kind.as_str() {
                "add" => LiquidityMoveKind::Add,
                "remove" => LiquidityMoveKind::Remove,
                other => {
                    return Err(RepositoryError::Integrity(format!(
                        "unknown liquidity move kind {other:?}"
                    )));
                }
            },
            amount_usd: convert_bigdecimal_to_decimal(row.amount_usd, "amount_usd")?,
        })
    }
}

impl TryFrom<ProtocolTvlRow> for ProtocolTvl {
    type Error = RepositoryError;

    fn try_from(row: ProtocolTvlRow) -> Result<Self, Self::Error> {
        Ok(ProtocolTvl {
            protocol: parse_protocol(&row.protocol)?,
            at: row.at,
            tvl_usd: usd(row.tvl_usd, "tvl_usd")?,
            pools_priced: row.pools_priced,
            pools_unvalued: row.pools_unvalued,
        })
    }
}

/// Folds the rows of [`LatestProtocolTvlRow`], ordered by protocol, into one TVL each.
pub(super) fn fold_latest(
    rows: Vec<LatestProtocolTvlRow>,
) -> RepositoryResult<Vec<LatestProtocolTvl>> {
    let mut latest: Vec<LatestProtocolTvl> = Vec::new();
    for row in rows {
        let discovered = match (
            row.discovered_period,
            row.discovered_usd,
            row.discovered_pools,
        ) {
            (Some(period), Some(tvl_usd), Some(pools)) => Some(DiscoveredTvl {
                window: parse_window(&period)?,
                tvl_usd: convert_bigdecimal_to_decimal(tvl_usd, "discovered_usd")?,
                pools,
            }),
            (None, None, None) => None,
            _ => {
                return Err(RepositoryError::Integrity(
                    "a discovered part read without all of its columns".to_owned(),
                ));
            }
        };
        let protocol = parse_protocol(&row.protocol)?;
        let current = match latest.last_mut() {
            Some(last) if last.tvl.protocol == protocol => last,
            _ => {
                latest.push(LatestProtocolTvl {
                    tvl: ProtocolTvl {
                        protocol,
                        at: row.at,
                        tvl_usd: usd(row.tvl_usd, "tvl_usd")?,
                        pools_priced: row.pools_priced,
                        pools_unvalued: row.pools_unvalued,
                    },
                    discovered: Vec::new(),
                });
                latest.last_mut().expect("just pushed")
            }
        };
        current.discovered.extend(discovered);
    }
    for tvl in &mut latest {
        tvl.discovered.sort_by_key(|d| d.window.hours());
    }
    Ok(latest)
}
