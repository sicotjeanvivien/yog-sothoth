mod cpamm_pool;
mod helius_das;
mod jupiter_price;
mod metrics;

use std::time::Duration;

pub(crate) use cpamm_pool::CpAmmPoolClient;
pub(crate) use helius_das::HeliusDasClient;
pub(crate) use jupiter_price::JupiterPriceClient;
pub(crate) use metrics::ProviderMetrics;

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
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HTTP_TOTAL_TIMEOUT)
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .build()
        .expect("static reqwest configuration is always buildable")
}
