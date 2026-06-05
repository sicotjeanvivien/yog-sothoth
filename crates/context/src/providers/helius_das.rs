//! Helius DAS API client — token metadata source.
//!
//! Calls the `getAssetBatch` JSON-RPC method to fetch identity for a
//! list of SPL mints. Returns the subset that yielded usable
//! metadata: a mint that DAS does not know about, or that lacks
//! `token_info.decimals`, is silently skipped — it will be retried on
//! the next poll cycle. Errors are returned typed; the caller (the
//! metadata worker) decides whether to swallow them or propagate.
//!
//! # Why getAssetBatch
//!
//! One HTTP round-trip per poll cycle, no matter how many mints are
//! missing — much friendlier on Helius rate limits than `getAsset`
//! per mint.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use tracing::warn;

use crate::{
    error::SourceError,
    source::{FetchedMetadata, MetadataSource},
};

/// Maximum number of IDs accepted by `getAssetBatch` in a single
/// call. Helius documents this as 1000. Currently a small allowlist
/// means a single chunk always suffices, but the worker still chunks
/// just in case the allowlist is later lifted.
const DAS_BATCH_MAX: usize = 1000;
const METADATA_SOURCE_TAG: &str = "helius_das";

// ── Wire types ────────────────────────────────────────────────────────

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
    /// Each entry corresponds (positionally) to an input id. May be
    /// `null` when the mint is unknown to DAS — hence `Option`.
    result: Vec<Option<DasAsset>>,
}

/// One asset entry. Only the fields the metadata worker needs are
/// deserialised — DAS returns a much larger object, the rest is
/// ignored.
#[derive(Debug, Deserialize)]
struct DasAsset {
    /// Mint address (base58). Returned as `id` by DAS.
    id: String,

    /// Display info. Often present even when `token_info` is, but
    /// kept optional defensively.
    content: Option<DasContent>,

    /// Token-specific info — the **only** place `decimals` is
    /// reported. Absent for non-fungible or oddball assets, which is
    /// the signal to skip the mint (no usable decimals).
    token_info: Option<DasTokenInfo>,
}

#[derive(Debug, Deserialize)]
struct DasContent {
    metadata: Option<DasMetadata>,
    /// Logo URI; may be an `ipfs://` URI. Stored verbatim.
    #[serde(default)]
    files: Vec<DasFile>,
    /// Some assets report the logo at `content.links.image`. Kept
    /// optional — the worker falls back to `files[0]` if absent.
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
    /// Decimal precision of the mint. The only mandatory field for
    /// us: a missing value means the asset is not a fungible SPL
    /// token in any useful sense, and we skip it.
    decimals: Option<u8>,
}

#[derive(Clone)]
pub struct HeliusDasClient {
    http: reqwest::Client,
    rpc_url: String,
}

impl HeliusDasClient {
    pub fn new(rpc_url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            rpc_url,
        }
    }

    /// Single HTTP call. Caller guarantees `mints.len() <= DAS_BATCH_MAX`.
    async fn fetch_chunk(&self, mints: &[Pubkey]) -> Result<Vec<FetchedMetadata>, SourceError> {
        if mints.is_empty() {
            return Ok(Vec::new());
        }
        debug_assert!(mints.len() <= DAS_BATCH_MAX);

        let ids: Vec<String> = mints.iter().map(|m| m.to_string()).collect();
        let request = DasRequest {
            jsonrpc: "2.0",
            id: "yog-context",
            method: "getAssetBatch",
            params: DasParams { ids: &ids },
        };

        let response = self
            .http
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| SourceError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| SourceError::Http(e.to_string()))?
            .json::<DasResponse>()
            .await
            .map_err(|e| SourceError::Decode(e.to_string()))?;

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
    /// Fetches metadata for an arbitrary number of mints, chunking
    /// internally to respect Helius' `getAssetBatch` 1000-id limit.
    /// Chunk-level failures are logged and skipped — the call returns
    /// whatever was successfully fetched.
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

/// Project a `DasAsset` into the worker's view, or drop it.
///
/// Drops the asset when:
///   - the mint string cannot be parsed back into a Pubkey
///     (extremely unlikely — DAS would have returned a malformed id);
///   - `token_info.decimals` is missing — the unique blocking
///     condition (the domain requires it).
fn into_fetched_metadata(asset: DasAsset) -> Option<FetchedMetadata> {
    let mint = Pubkey::try_from(asset.id.as_str()).ok()?;
    let decimals = asset.token_info.as_ref().and_then(|ti| ti.decimals)?;

    let (symbol, name) = asset
        .content
        .as_ref()
        .and_then(|c| c.metadata.as_ref())
        .map(|m| (m.symbol.clone(), m.name.clone()))
        .unwrap_or((None, None));

    let logo_uri = asset.content.as_ref().and_then(|c| {
        c.links
            .as_ref()
            .and_then(|l| l.image.clone())
            .or_else(|| c.files.iter().find_map(|f| f.uri.clone()))
    });

    Some(FetchedMetadata {
        mint,
        symbol,
        name,
        decimals,
        logo_uri,
        metadata_source: METADATA_SOURCE_TAG.to_string(),
    })
}

#[cfg(test)]
#[path = "helius_das_tests.rs"]
mod tests;
