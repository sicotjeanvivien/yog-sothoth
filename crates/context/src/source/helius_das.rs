//! Helius DAS API client — token metadata source.
//!
//! STUB (commit 1): the client struct and its own reqwest client
//! exist so `AppState` can hold it and the crate compiles. The
//! actual `getAsset` / `getAssetBatch` calls and the response types
//! land in commit 2 (metadata worker).

use reqwest::Client;

/// Client for the Helius Digital Asset Standard (DAS) API.
///
/// Owns its own `reqwest::Client`, separate from the Jupiter client
/// and from the indexer's RPC client.
#[derive(Clone)]
pub struct HeliusDasClient {
    #[allow(dead_code)] // wired in commit 2
    http: Client,
    #[allow(dead_code)] // wired in commit 2
    base_url: String,
}

impl HeliusDasClient {
    /// Build the client against the given Helius RPC base URL.
    pub fn new(base_url: String) -> Self {
        Self {
            http: Client::new(),
            base_url,
        }
    }

    // fetch_asset_batch(...) — added in commit 2.
}
