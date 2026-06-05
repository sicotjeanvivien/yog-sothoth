//! Jupiter price API client — token USD price source.
//!
//! Calls Price API V3 (`GET https://api.jup.ag/price/v3?ids=...`)
//! and returns the subset of mints that yielded a usable price.
//! Mints that Jupiter cannot price (untraded recently, flagged by
//! their heuristics) are silently dropped: this is documented V3
//! behaviour and not an error.

use std::collections::HashMap;

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::Deserialize;
use solana_pubkey::Pubkey;
use tracing::warn;
use yog_core::domain::PriceProvider;

use crate::{
    error::SourceError,
    source::{FetchedPrice, PriceSource},
};

/// Maximum number of `ids` accepted by Price API V3 in a single
/// call. Documented limit: 50.
const JUPITER_BATCH_MAX: usize = 50;

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

    /// Single HTTP call. Caller guarantees `mints.len() <= JUPITER_BATCH_MAX`.
    async fn fetch_chunk(&self, mints: &[Pubkey]) -> Result<Vec<FetchedPrice>, SourceError> {
        if mints.is_empty() {
            return Ok(Vec::new());
        }
        debug_assert!(mints.len() <= JUPITER_BATCH_MAX);

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

        Ok(response
            .into_iter()
            .filter_map(into_fetched_price)
            .collect())
    }
}

#[async_trait]
impl PriceSource for JupiterPriceClient {
    /// Fetches USD prices for an arbitrary number of mints, chunking
    /// internally on Jupiter's 50-id limit. Chunk-level failures are
    /// logged and skipped.
    async fn fetch_prices(&self, mints: &[Pubkey]) -> Result<Vec<FetchedPrice>, SourceError> {
        let mut all = Vec::with_capacity(mints.len());
        for chunk in mints.chunks(JUPITER_BATCH_MAX) {
            match self.fetch_chunk(chunk).await {
                Ok(fetched) => all.extend(fetched),
                Err(e) => {
                    warn!(
                        error = %e,
                        chunk_size = chunk.len(),
                        "jupiter_price: chunk failed, continuing",
                    );
                }
            }
        }
        Ok(all)
    }
}

/// Project one HashMap entry from Jupiter's response into the
/// worker's view, or drop it. Drops when:
///   - the entry has no usable `usdPrice` (null or absent);
///   - the mint string cannot be parsed back into a Pubkey
///     (unlikely — Jupiter would have to return a malformed id).
fn into_fetched_price((mint_str, entry): (String, JupiterPriceEntry)) -> Option<FetchedPrice> {
    let price_usd = entry.usd_price?;
    let mint = Pubkey::try_from(mint_str.as_str()).ok()?;
    Some(FetchedPrice {
        mint,
        price_provider: PriceProvider::Jupiter,
        price_usd,
    })
}

#[cfg(test)]
#[path = "jupiter_price_tests.rs"]
mod tests;
