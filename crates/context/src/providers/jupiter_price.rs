//! Jupiter price API client — token USD price source.
//!
//! Calls Price API V3 (`GET https://api.jup.ag/price/v3?ids=...`)
//! and returns the subset of mints that yielded a usable price.
//! Mints that Jupiter cannot price (untraded recently, flagged by
//! their heuristics) are silently dropped: this is documented V3
//! behaviour and not an error.
//!
//! Chunks beyond the first can hit Jupiter's rate limit (429) because
//! they are sent back-to-back; those are retried a bounded number of
//! times, pacing on the `Retry-After` header when present.

use super::metrics::ProviderMetrics;
use std::collections::HashMap;
use std::time::{Duration, Instant};

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

/// Attempts per chunk when Jupiter answers 429 (1 initial call +
/// retries). Chunks are sent back-to-back, so the first chunks of a
/// tick can exhaust the per-second budget and 429 the rest; a short
/// paced retry recovers them instead of losing a price sample.
const RATE_LIMIT_MAX_ATTEMPTS: u32 = 3;

/// Backoff before retry attempt `n` (0-based) when the 429 carried
/// no `Retry-After` header: 1s, then 2s.
const RATE_LIMIT_BASE_BACKOFF: Duration = Duration::from_secs(1);

/// Upper bound on any single retry sleep, including a server-provided
/// `Retry-After`. Keeps one bad header from stalling the worker (and
/// its graceful shutdown) for minutes.
const RATE_LIMIT_MAX_BACKOFF: Duration = Duration::from_secs(10);

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
    /// Jupiter API base URL (e.g. `https://api.jup.ag`); `/price/v3`
    /// is appended per request.
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
        let start = Instant::now();
        let result = self.fetch_chunk_inner(mints).await;
        let elapsed = start.elapsed().as_secs_f64();

        let outcome = match &result {
            Ok(_) => "ok",
            Err(SourceError::Http(_)) => "http",
            Err(SourceError::RateLimited { .. }) => "rate_limited",
            Err(SourceError::Decode(_)) => "decode",
        };
        ProviderMetrics::record_call(PriceProvider::Jupiter.as_str(), outcome, elapsed);

        result
    }

    async fn fetch_chunk_inner(&self, mints: &[Pubkey]) -> Result<Vec<FetchedPrice>, SourceError> {
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
            .map_err(|e| SourceError::Http(e.to_string()))?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited {
                retry_after: parse_retry_after(response.headers()),
            });
        }

        let response = response
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

impl JupiterPriceClient {
    /// One chunk with bounded 429 retries: sleeps `Retry-After` when
    /// Jupiter provided it, exponential backoff otherwise, then gives
    /// the chunk up (skip-and-log in the caller). Other errors are
    /// not retried — they are not pacing problems.
    async fn fetch_chunk_with_retry(
        &self,
        mints: &[Pubkey],
    ) -> Result<Vec<FetchedPrice>, SourceError> {
        let mut attempt = 0;
        loop {
            match self.fetch_chunk(mints).await {
                Err(SourceError::RateLimited { retry_after })
                    if attempt + 1 < RATE_LIMIT_MAX_ATTEMPTS =>
                {
                    let delay = rate_limit_backoff(attempt, retry_after);
                    warn!(
                        attempt,
                        delay_ms = delay.as_millis() as u64,
                        chunk_size = mints.len(),
                        "jupiter_price: rate-limited, backing off before retry",
                    );
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
                result => return result,
            }
        }
    }
}

#[async_trait]
impl PriceSource for JupiterPriceClient {
    /// Fetches USD prices for an arbitrary number of mints, chunking
    /// internally on Jupiter's 50-id limit. Rate-limited chunks are
    /// retried with backoff; chunk-level failures are logged and
    /// skipped.
    async fn fetch_prices(&self, mints: &[Pubkey]) -> Result<Vec<FetchedPrice>, SourceError> {
        let mut all = Vec::with_capacity(mints.len());
        for chunk in mints.chunks(JUPITER_BATCH_MAX) {
            match self.fetch_chunk_with_retry(chunk).await {
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

/// Delay before retry attempt `attempt` (0-based): the server's
/// `Retry-After` when present, exponential backoff on
/// `RATE_LIMIT_BASE_BACKOFF` otherwise — both capped at
/// `RATE_LIMIT_MAX_BACKOFF`.
fn rate_limit_backoff(attempt: u32, retry_after: Option<Duration>) -> Duration {
    retry_after
        .unwrap_or_else(|| RATE_LIMIT_BASE_BACKOFF * 2u32.saturating_pow(attempt))
        .min(RATE_LIMIT_MAX_BACKOFF)
}

/// Extract the `Retry-After` header as a duration. Only the
/// delta-seconds form is handled; the HTTP-date form (which Jupiter
/// does not use) yields `None` and falls back to our own backoff.
fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
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
