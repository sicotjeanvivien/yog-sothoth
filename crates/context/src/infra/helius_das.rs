//! Helius DAS client, the token metadata source: one `getAssetBatch` call per chunk rather than a
//! `getAsset` per mint. A mint DAS does not know, or without `token_info.decimals`, is skipped and
//! retried next cycle; errors are returned typed.

use super::metrics::ProviderMetrics;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use std::time::Instant;
use tracing::warn;

use yog_bootstrap::SecretUrl;
use yog_core::domain::MetadataProvider;

use crate::{
    application::source::{FetchedMetadata, MetadataSource},
    error::SourceError,
};

/// Maximum number of IDs `getAssetBatch` accepts in one call, as Helius documents it.
const DAS_BATCH_MAX: usize = 1000;
const PROVIDER_LABEL: &str = "helius_das";

/// JSON-RPC request body for `getAssetBatch`.
#[derive(Debug, Serialize)]
struct DasRequest<'a> {
    jsonrpc: &'static str,
    id: &'static str,
    method: &'static str,
    params: DasParams<'a>,
}

#[derive(Debug, Serialize)]
struct DasParams<'a> {
    ids: &'a [String],
}

/// JSON-RPC envelope returned by Helius.
#[derive(Debug, Deserialize)]
struct DasResponse {
    /// One entry per input id, in order; `null` when DAS does not know the mint.
    result: Vec<Option<DasAsset>>,
}

/// One asset entry, reduced to the fields the metadata worker needs.
#[derive(Debug, Deserialize)]
struct DasAsset {
    /// Mint address (base58), returned as `id`.
    id: String,

    /// Display info, kept optional defensively.
    content: Option<DasContent>,

    /// Token-specific info, the only place `decimals` is reported; absent for a non-fungible
    /// asset, which is then skipped.
    token_info: Option<DasTokenInfo>,
}

#[derive(Debug, Deserialize)]
struct DasContent {
    metadata: Option<DasMetadata>,
    /// Logo URI; may be an `ipfs://` URI. Stored verbatim.
    #[serde(default)]
    files: Vec<DasFile>,
    /// Some assets report the logo here; the worker falls back to `files[0]` when absent.
    links: Option<DasLinks>,
}

#[derive(Debug, Deserialize)]
struct DasMetadata {
    name: Option<String>,
    symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DasFile {
    uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DasLinks {
    image: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DasTokenInfo {
    /// Decimal precision of the mint, the one mandatory field: without it the asset is skipped.
    decimals: Option<u8>,
}

#[derive(Clone)]
pub struct HeliusDasClient {
    http: reqwest::Client,
    /// ⚠️ Stays wrapped for the client's whole life: the provider key sits in this URL's query string.
    rpc_url: SecretUrl,
}

impl HeliusDasClient {
    pub fn new(rpc_url: SecretUrl) -> Self {
        Self {
            http: super::http_client(),
            rpc_url,
        }
    }

    /// Single HTTP call. Caller guarantees `mints.len() <= DAS_BATCH_MAX`.
    async fn fetch_chunk(&self, mints: &[Pubkey]) -> Result<Vec<FetchedMetadata>, SourceError> {
        if mints.is_empty() {
            return Ok(Vec::new());
        }
        debug_assert!(mints.len() <= DAS_BATCH_MAX);
        let start = Instant::now();
        let result = self.fetch_chunk_inner(mints).await;
        let elapsed = start.elapsed().as_secs_f64();

        let outcome = match &result {
            Ok(_) => "ok",
            Err(SourceError::Http(_)) => "http",
            Err(SourceError::RateLimited { .. }) => "rate_limited",
            Err(SourceError::Decode(_)) => "decode",
        };
        ProviderMetrics::record_call(PROVIDER_LABEL, outcome, elapsed);

        result
    }

    /// The HTTP work itself, kept apart so `fetch_chunk` wraps it with metrics.
    async fn fetch_chunk_inner(
        &self,
        mints: &[Pubkey],
    ) -> Result<Vec<FetchedMetadata>, SourceError> {
        let ids: Vec<String> = mints.iter().map(|m| m.to_string()).collect();
        let request = DasRequest {
            jsonrpc: "2.0",
            id: "yog-context",
            method: "getAssetBatch",
            params: DasParams { ids: &ids },
        };

        let response = self
            .http
            .post(self.rpc_url.expose())
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<DasResponse>()
            .await?;

        Ok(response
            .result
            .into_iter()
            .flatten()
            .filter_map(into_fetched_metadata)
            .collect())
    }
}

#[async_trait]
impl MetadataSource for HeliusDasClient {
    /// Fetches metadata for any number of mints, chunked under `DAS_BATCH_MAX`; a failed chunk is
    /// logged and skipped, the call returning whatever was fetched.
    async fn fetch_metadata(&self, mints: &[Pubkey]) -> Result<Vec<FetchedMetadata>, SourceError> {
        let mut all = Vec::with_capacity(mints.len());
        for chunk in mints.chunks(DAS_BATCH_MAX) {
            match self.fetch_chunk(chunk).await {
                Ok(fetched) => all.extend(fetched),
                Err(e) => {
                    warn!(
                        error = %e,
                        chunk_size = chunk.len(),
                        "helius_das: chunk failed, continuing",
                    );
                }
            }
        }
        Ok(all)
    }
}

/// Project a `DasAsset` into the worker's view, or drop it: an unparseable mint, or no
/// `token_info.decimals`.
fn into_fetched_metadata(asset: DasAsset) -> Option<FetchedMetadata> {
    let mint = Pubkey::try_from(asset.id.as_str()).ok()?;
    let decimals = asset.token_info.as_ref().and_then(|ti| ti.decimals)?;

    let (symbol, name) = asset
        .content
        .as_ref()
        .and_then(|c| c.metadata.as_ref())
        .map(|m| (m.symbol.clone(), m.name.clone()))
        .unwrap_or((None, None));

    // Filter empty strings before the fallback: an `image: Some("")` must not
    // shadow a valid `files[].uri`, and absence is represented as None.
    let logo_uri = asset.content.as_ref().and_then(|c| {
        c.links
            .as_ref()
            .and_then(|l| l.image.clone())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                c.files
                    .iter()
                    .find_map(|f| f.uri.clone().filter(|s| !s.is_empty()))
            })
    });

    Some(FetchedMetadata {
        mint,
        symbol,
        name,
        decimals,
        logo_uri,
        metadata_provider: MetadataProvider::HeliusDas,
    })
}

#[cfg(test)]
#[path = "helius_das_tests.rs"]
mod tests;
