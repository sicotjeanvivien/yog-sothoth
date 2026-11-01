//! The adapters: one HTTP client per provider (Helius DAS, Jupiter, Solana
//! RPC), each implementing a port of `application::source`; their metrics;
//! the shared `http_client` and its timeouts; and the conversion of a
//! `reqwest` failure into a `SourceError`, which strips the URL.

mod helius_das;
mod jupiter_price;
mod metrics;
#[cfg(test)]
mod scripted_provider;
mod solana_account;
mod source_error;

use std::time::Duration;

pub(crate) use helius_das::HeliusDasClient;
pub(crate) use jupiter_price::JupiterPriceClient;
pub(crate) use metrics::ProviderMetrics;
pub(crate) use solana_account::SolanaAccountClient;

/// Overall per-request deadline shared by the provider clients.
/// ⚠️ `reqwest` has no default timeout: a stalled provider would hang the tick for ever while the
/// process looks alive.
const HTTP_TOTAL_TIMEOUT: Duration = Duration::from_secs(15);

/// TCP/TLS connect deadline: fail fast on an unreachable provider.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Build a provider HTTP client carrying the shared timeouts.
pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HTTP_TOTAL_TIMEOUT)
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .build()
        .expect("static reqwest configuration is always buildable")
}
