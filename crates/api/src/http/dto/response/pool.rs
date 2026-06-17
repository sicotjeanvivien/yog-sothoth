use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use yog_core::domain::{Pool, PoolAnalytics};

use crate::{
    application::{EnrichedPool, EnrichedToken},
    http::dto::EmbeddedTokenResponse,
};

/// Wire shape of a pool in API responses.
///
/// Independent from the domain `Pool` so the public contract can evolve
/// (rename `pool_address` → `address`, etc.) without breaking internal
/// representations. Pubkeys are formatted as base58, protocol as
/// snake_case (matching its `Serialize` impl).
///
/// Analytics (TVL, 24h volume) are denominated in USD. They are
/// `Option` because their computation requires data that may not be
/// available yet (no current state, no priced token, no swap in the
/// window). Serialised as JSON numbers via `rust_decimal`'s exact
/// decimal representation — consistent with the price block in
/// `EmbeddedPriceResponse`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolResponse {
    pub(crate) pool_address: String,
    pub(crate) protocol: String,
    pub(crate) token_a: EmbeddedTokenResponse,
    pub(crate) token_b: EmbeddedTokenResponse,
    /// Base trading fee in basis points (genesis fee tier). `None` until the
    /// pool's `InitializePool` event has been indexed.
    pub(crate) fee_bps: Option<Decimal>,
    /// Fee-split percents (0..=100) from the on-chain pool account: Meteora's,
    /// a partner's, and a referrer's cut of the trading fee. `None` until
    /// yog-context resolves the pool account.
    pub(crate) protocol_fee_percent: Option<u8>,
    pub(crate) partner_fee_percent: Option<u8>,
    pub(crate) referral_fee_percent: Option<u8>,
    pub(crate) tvl_usd: Option<Decimal>,
    pub(crate) volume_24h_usd: Option<Decimal>,
    /// Realized trading fee over the last 24h (USD), and its split: Meteora's
    /// cut, the LP cut (`fees - protocol`). `None` under the same partial-price
    /// coverage rules as `volume_24h_usd`.
    pub(crate) fees_24h_usd: Option<Decimal>,
    pub(crate) protocol_fees_24h_usd: Option<Decimal>,
    pub(crate) lp_fees_24h_usd: Option<Decimal>,
    /// Effective realized fee rate in basis points (`fees / volume * 10000`)
    /// over the 24h window. `None` when volume is absent or zero.
    pub(crate) effective_fee_bps: Option<Decimal>,
    pub(crate) first_seen_at: DateTime<Utc>,
    pub(crate) last_seen_at: DateTime<Utc>,
}

/// Effective realized fee rate in basis points over the window:
/// `fees / volume * 10_000`. `None` when volume is unknown or zero (no
/// meaningful rate, and avoids a division by zero).
fn effective_fee_bps(fees_usd: Option<Decimal>, volume_usd: Option<Decimal>) -> Option<Decimal> {
    match (fees_usd, volume_usd) {
        (Some(fees), Some(volume)) if !volume.is_zero() => {
            Some(fees / volume * Decimal::from(10_000))
        }
        _ => None,
    }
}

impl PoolResponse {
    /// Compose the pool with its two enriched token sides and the
    /// derived analytics. The caller (the pool handler) is
    /// responsible for fetching the analytics for the requested
    /// pools in batch — see `enrich_pool` in `handlers/pools.rs`.
    pub(crate) fn new(
        pool: Pool,
        token_a: EmbeddedTokenResponse,
        token_b: EmbeddedTokenResponse,
        analytics: PoolAnalytics,
    ) -> Self {
        Self {
            pool_address: pool.pool_address.to_string(),
            protocol: pool.protocol.to_string(),
            token_a,
            token_b,
            fee_bps: pool.fee_bps,
            protocol_fee_percent: pool.protocol_fee_percent,
            partner_fee_percent: pool.partner_fee_percent,
            referral_fee_percent: pool.referral_fee_percent,
            tvl_usd: analytics.tvl_usd,
            volume_24h_usd: analytics.volume_24h_usd,
            fees_24h_usd: analytics.fees_24h_usd,
            protocol_fees_24h_usd: analytics.protocol_fees_24h_usd,
            // LP share = total realized fee minus the protocol's cut.
            lp_fees_24h_usd: match (analytics.fees_24h_usd, analytics.protocol_fees_24h_usd) {
                (Some(fees), Some(protocol)) => Some(fees - protocol),
                _ => None,
            },
            effective_fee_bps: effective_fee_bps(analytics.fees_24h_usd, analytics.volume_24h_usd),
            first_seen_at: pool.first_seen_at,
            last_seen_at: pool.last_seen_at,
        }
    }
}

impl From<EnrichedToken> for EmbeddedTokenResponse {
    fn from(t: EnrichedToken) -> Self {
        EmbeddedTokenResponse::from_sources(t.mint, t.metadata, t.price)
    }
}

impl From<EnrichedPool> for PoolResponse {
    fn from(e: EnrichedPool) -> Self {
        PoolResponse::new(e.pool, e.token_a.into(), e.token_b.into(), e.analytics)
    }
}

#[cfg(test)]
mod tests {
    use super::effective_fee_bps;
    use rust_decimal::Decimal;

    #[test]
    fn effective_fee_bps_is_fees_over_volume_in_bps() {
        // 25 USD fees on 10_000 USD volume = 0.25% = 25 bps.
        let bps = effective_fee_bps(Some(Decimal::new(25, 0)), Some(Decimal::new(10_000, 0)));
        assert_eq!(bps, Some(Decimal::new(25, 0)));
    }

    #[test]
    fn effective_fee_bps_none_when_volume_zero() {
        assert_eq!(
            effective_fee_bps(Some(Decimal::new(25, 0)), Some(Decimal::ZERO)),
            None
        );
    }

    #[test]
    fn effective_fee_bps_none_when_an_input_missing() {
        assert_eq!(effective_fee_bps(None, Some(Decimal::new(10, 0))), None);
        assert_eq!(effective_fee_bps(Some(Decimal::new(10, 0)), None), None);
    }
}
