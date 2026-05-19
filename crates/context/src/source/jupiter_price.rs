//! Jupiter price API client — token price source.
//!
//! STUB (commit 1): the client struct and its own reqwest client
//! exist so `AppState` can hold it and the crate compiles. The
//! actual price-fetch call and the response types land in commit 3
//! (price worker).

use reqwest::Client;

/// Client for the Jupiter price API.
///
/// Owns its own `reqwest::Client`, separate from the Helius client.
#[derive(Clone)]
pub struct JupiterPriceClient {
    #[allow(dead_code)] // wired in commit 3
    http: Client,
    #[allow(dead_code)] // wired in commit 3
    base_url: String,
}

impl JupiterPriceClient {
    /// Build the client against the given Jupiter API base URL.
    pub fn new(base_url: String) -> Self {
        Self {
            http: Client::new(),
            base_url,
        }
    }

    // fetch_prices_batch(...) — added in commit 3.
}
