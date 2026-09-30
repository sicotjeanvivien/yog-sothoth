//! The adapters: one HTTP client per provider (Helius DAS, Jupiter, Solana
//! RPC), each implementing a port of `application::source`; their metrics;
//! the shared `http_client` and its timeouts; and the conversion of a
//! `reqwest` failure into a `SourceError`, which strips the URL.

mod helius_das;
mod jupiter_price;
mod metrics;
mod solana_account;
mod source_error;

use std::time::Duration;

pub(crate) use helius_das::HeliusDasClient;
pub(crate) use jupiter_price::JupiterPriceClient;
pub(crate) use metrics::ProviderMetrics;
pub(crate) use solana_account::SolanaAccountClient;

/// Overall per-request deadline shared by the provider clients.
///
/// Without it (`reqwest`'s default is *no* timeout) a stalled provider
/// response hangs the worker's tick forever with the process still
/// alive — invisible to Docker's restart policy, enrichment silently
/// dead. With it, a hang degrades into a tick-level `SourceError`
/// already absorbed by the workers' skip-and-log.
const HTTP_TOTAL_TIMEOUT: Duration = Duration::from_secs(15);

/// TCP/TLS connect deadline — fail fast on an unreachable provider
/// instead of consuming the whole request budget.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Build a provider HTTP client carrying the shared timeouts.
pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HTTP_TOTAL_TIMEOUT)
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .build()
        .expect("static reqwest configuration is always buildable")
}
