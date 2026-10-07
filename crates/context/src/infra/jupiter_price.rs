//! Jupiter Price API V3 client (`GET …/price/v3?ids=…`): says, for every mint
//! of a chunk Jupiter answered, whether it came back with a price. Chunks are
//! spaced under the key's rate limit, and the pause between two is where the
//! stop is heard; one refused on 429 anyway is retried a bounded number of
//! times.

use super::metrics::ProviderMetrics;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::Deserialize;
use solana_pubkey::Pubkey;
use tokio_util::sync::CancellationToken;
use yog_bootstrap::SecretKey;
use yog_core::domain::PriceProvider;

use crate::{
    application::source::{FetchedPrice, PriceAnswer, PriceSource},
    error::SourceError,
};

mod log;

/// Maximum number of `ids` per call (documented limit).
const JUPITER_BATCH_MAX: usize = 50;

/// Attempts per chunk when Jupiter answers 429, the first call included.
const RATE_LIMIT_MAX_ATTEMPTS: u32 = 3;

/// Backoff before retry `n` (0-based) without `Retry-After`: 1 s, then 2 s.
const RATE_LIMIT_BASE_BACKOFF: Duration = Duration::from_secs(1);

/// Cap on any retry sleep, a server's `Retry-After` included, so one bad
/// header cannot stall the worker and its shutdown.
const RATE_LIMIT_MAX_BACKOFF: Duration = Duration::from_secs(10);

/// What [`spacing_under`] divides by the rate limit: a minute and a tenth,
/// so the client sends ten requests where the limit allows eleven.
const SPACING_PER_REQUEST_ALLOWED: Duration = Duration::from_secs(66);

// ── Wire types ────────────────────────────────────────────────────────

/// One price entry from Jupiter's V3 response, reduced to `usdPrice`.
///
/// ⚠️ `Option` **and** `#[serde(default)]`: Jupiter sends `null` or omits the
/// field for a mint it cannot price, and without `default` the omission would
/// fail to deserialise.
#[derive(Debug, Deserialize)]
struct JupiterPriceEntry {
    #[serde(rename = "usdPrice", default)]
    usd_price: Option<Decimal>,
}

// ── Client ────────────────────────────────────────────────────────────

/// Client for the Jupiter Price API V3, authenticated by the `x-api-key`
/// header.
#[derive(Clone)]
pub struct JupiterPriceClient {
    http: reqwest::Client,
    /// Base URL (e.g. `https://api.jup.ag`). No secret in it — the key goes in
    /// a header — hence a plain `String`.
    base_url: String,
    /// Stays a [`SecretKey`] until the header is built.
    api_key: SecretKey,
    /// The least time between the starts of two chunks.
    request_spacing: Duration,
}

impl JupiterPriceClient {
    /// Build the client against the given base URL and API key, spaced under
    /// `rate_limit`, the requests per minute the key's tier allows.
    pub fn new(base_url: String, api_key: SecretKey, rate_limit: NonZeroU32) -> Self {
        Self {
            http: super::http_client(),
            base_url,
            api_key,
            request_spacing: spacing_under(rate_limit),
        }
    }

    /// The least time between the starts of two chunks.
    pub fn request_spacing(&self) -> Duration {
        self.request_spacing
    }

    /// Single HTTP call. Caller guarantees `mints.len() <= JUPITER_BATCH_MAX`.
    async fn fetch_chunk(&self, mints: &[Pubkey]) -> Result<PriceAnswer, SourceError> {
        if mints.is_empty() {
            return Ok(PriceAnswer::default());
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

    async fn fetch_chunk_inner(&self, mints: &[Pubkey]) -> Result<PriceAnswer, SourceError> {
        let ids: String = mints
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let url = format!("{}/price/v3?ids={}", self.base_url, ids);

        let response = self
            .http
            .get(&url)
            .header("x-api-key", self.api_key.expose())
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited {
                retry_after: parse_retry_after(response.headers()),
            });
        }

        let response = response
            .error_for_status()?
            .json::<HashMap<String, JupiterPriceEntry>>()
            .await?;

        let priced = response
            .into_iter()
            .filter_map(into_fetched_price)
            .collect();

        match chunk_answer(mints, priced) {
            Some(answer) => Ok(answer),
            None => {
                log::degraded_answer(mints.len());
                Ok(PriceAnswer::default())
            }
        }
    }
}

impl JupiterPriceClient {
    /// One chunk with bounded 429 retries, then given up. Other errors are not
    /// retried: they are not pacing problems.
    async fn fetch_chunk_with_retry(&self, mints: &[Pubkey]) -> Result<PriceAnswer, SourceError> {
        let mut attempt = 0;
        loop {
            match self.fetch_chunk(mints).await {
                Err(SourceError::RateLimited { retry_after })
                    if attempt + 1 < RATE_LIMIT_MAX_ATTEMPTS =>
                {
                    let delay = rate_limit_backoff(attempt, retry_after);
                    log::rate_limited(attempt, delay, mints.len());
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
    /// Fetches the prices chunk by chunk, one every `request_spacing`. A chunk
    /// that fails is logged and skipped: its mints are in neither list of the
    /// answer.
    async fn fetch_prices(
        &self,
        mints: &[Pubkey],
        shutdown: &CancellationToken,
    ) -> Result<PriceAnswer, SourceError> {
        let mut answer = PriceAnswer::default();
        let mut next_start = tokio::time::Instant::now();
        for (index, chunk) in mints.chunks(JUPITER_BATCH_MAX).enumerate() {
            // ⚠️ The only place the stop is heard: a request in flight ends,
            // and its answer is kept.
            tokio::select! {
                biased;

                () = shutdown.cancelled() => {
                    log::stopped(index * JUPITER_BATCH_MAX, mints.len());
                    break;
                }
                () = tokio::time::sleep_until(next_start) => {}
            }
            // ⚠️ From start to start: a pause after the answer would add the
            // request's own latency to every chunk.
            next_start = tokio::time::Instant::now() + self.request_spacing;
            match self.fetch_chunk_with_retry(chunk).await {
                Ok(answered) => {
                    answer.priced.extend(answered.priced);
                    answer.unpriced.extend(answered.unpriced);
                }
                Err(e) => {
                    log::chunk_failed(&e, chunk.len());
                }
            }
        }
        Ok(answer)
    }
}

/// The answer to one chunk: every mint asked that did not come back with a
/// price is unpriced (a missing entry included).
///
/// ⚠️ `None` when not one came back with a price: Jupiter answers for the mints
/// it cannot price too, and a chunk mixes live and dead mints, so `{}`, an
/// error body or all-null entries are a degraded answer. Read as a verdict,
/// they would hold back every live mint of the chunk.
fn chunk_answer(asked: &[Pubkey], priced: Vec<FetchedPrice>) -> Option<PriceAnswer> {
    let with_price: HashSet<Pubkey> = priced.iter().map(|price| price.mint).collect();
    if !asked.iter().any(|mint| with_price.contains(mint)) {
        return None;
    }

    let unpriced = asked
        .iter()
        .filter(|mint| !with_price.contains(mint))
        .copied()
        .collect();

    Some(PriceAnswer { priced, unpriced })
}

/// The least time between the starts of two chunks, under `rate_limit`.
///
/// ⚠️ Under, not at: Jupiter counts over a sliding minute, on arrival, and
/// two requests can arrive closer than they left.
fn spacing_under(rate_limit: NonZeroU32) -> Duration {
    SPACING_PER_REQUEST_ALLOWED / rate_limit.get()
}

/// Delay before retry `attempt`: `Retry-After` if any, exponential backoff
/// otherwise, capped at `RATE_LIMIT_MAX_BACKOFF`.
fn rate_limit_backoff(attempt: u32, retry_after: Option<Duration>) -> Duration {
    retry_after
        .unwrap_or_else(|| RATE_LIMIT_BASE_BACKOFF * 2u32.saturating_pow(attempt))
        .min(RATE_LIMIT_MAX_BACKOFF)
}

/// `Retry-After` in its delta-seconds form; the HTTP-date form yields `None`.
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

/// One response entry as a price, or `None` without a usable `usdPrice` or a
/// parseable mint.
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
