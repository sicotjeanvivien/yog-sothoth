//! Jupiter price API client — token USD price source.
//!
//! Calls Price API V3 (`GET https://api.jup.ag/price/v3?ids=...`)
//! and returns the subset of mints that yielded a usable price.
//! Mints that Jupiter cannot price (untraded recently, flagged by
//! their heuristics) are silently dropped: this is documented V3
//! behaviour and not an error.

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::Deserialize;
use solana_pubkey::Pubkey;

use crate::error::SourceError;

/// Maximum number of `ids` accepted by Price API V3 in a single
/// call. Documented limit: 50.
pub const JUPITER_BATCH_MAX: usize = 50;

// ── Wire types ────────────────────────────────────────────────────────

/// One price entry from Jupiter's V3 response.
///
/// The API returns several fields per entry (createdAt, liquidity,
/// blockId, decimals, priceChange24h, launchpad, etc.); we only
/// deserialise `usdPrice` and let serde ignore the rest.
///
/// `usd_price` is wrapped in `Option` AND marked `#[serde(default)]`
/// because Jupiter has three real-world cases for unpriced mints:
///   - `usdPrice: <number>` — Some(value),
///   - `usdPrice: null`     — None,
///   - the field is entirely ABSENT from the entry — also None
///     (without `default`, serde would error on the missing field).
#[derive(Debug, Deserialize)]
struct JupiterPriceEntry {
    #[serde(rename = "usdPrice", default)]
    usd_price: Option<Decimal>,
}

// ── Returned shape ────────────────────────────────────────────────────

/// A successfully fetched price, ready to be turned into the domain
/// `TokenPrice` by the worker.
#[derive(Debug, Clone)]
pub struct FetchedPrice {
    pub mint: Pubkey,
    pub price_usd: Decimal,
}

// ── Client ────────────────────────────────────────────────────────────

/// Client for the Jupiter Price API V3.
///
/// Owns its own `reqwest::Client`, separate from the Helius client
/// and from the indexer's RPC client. Holds the API key set in the
/// `x-api-key` header on every request.
#[derive(Clone)]
pub struct JupiterPriceClient {
    http: reqwest::Client,
    /// Price API V3 base URL (e.g. `https://api.jup.ag/price/v3`).
    base_url: String,
    /// API key — sent on every request via `x-api-key`.
    api_key: String,
}

impl JupiterPriceClient {
    /// Build the client against the given base URL and API key.
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            api_key,
        }
    }

    /// Fetch USD prices for a batch of mints.
    ///
    /// `mints` must not exceed `JUPITER_BATCH_MAX`; the worker
    /// chunks the queue before calling.
    ///
    /// Returns the subset of mints that Jupiter actually priced.
    /// Mints whose entry has no usable `usdPrice` are dropped —
    /// they will be retried on the next tick, with the same likely
    /// outcome until they become tradable again.
    pub async fn fetch_prices_batch(
        &self,
        mints: &[Pubkey],
    ) -> Result<Vec<FetchedPrice>, SourceError> {
        if mints.is_empty() {
            return Ok(Vec::new());
        }
        debug_assert!(mints.len() <= JUPITER_BATCH_MAX);

        // Build the `ids=mint1,mint2,...` query string. reqwest's
        // `.query()` would percent-encode the commas; the V3 API
        // expects them raw, so we build the URL ourselves.
        let ids: String = mints
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let url = format!("{}/price/v3?ids={}", self.base_url, ids);

        let response = self
            .http
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await
            .map_err(|e| SourceError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| SourceError::Http(e.to_string()))?
            .json::<HashMap<String, JupiterPriceEntry>>()
            .await
            .map_err(|e| SourceError::Decode(e.to_string()))?;

        // Project: drop entries without a usable price, parse the
        // mint string back into a Pubkey.
        Ok(response
            .into_iter()
            .filter_map(|(mint_str, entry)| {
                let price = entry.usd_price?;
                let mint = Pubkey::try_from(mint_str.as_str()).ok()?;
                Some(FetchedPrice {
                    mint,
                    price_usd: price,
                })
            })
            .collect())
    }
}
