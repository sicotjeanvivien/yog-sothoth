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

use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::error::SourceError;

/// Maximum number of IDs accepted by `getAssetBatch` in a single
/// call. Helius documents this as 1000. Currently a small allowlist
/// means a single chunk always suffices, but the worker still chunks
/// just in case the allowlist is later lifted.
pub const DAS_BATCH_MAX: usize = 1000;

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

// ── Returned shape ────────────────────────────────────────────────────

/// A successfully fetched piece of metadata, ready to be turned into
/// the domain `TokenMetadata` by the worker.
///
/// This is the source-layer view: it carries the bits the worker
/// needs, but does NOT carry timestamps or the `metadata_source` tag
/// — those are added by the worker when building the domain object.
#[derive(Debug, Clone)]
pub struct FetchedMetadata {
    pub mint: Pubkey,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub decimals: u8,
    pub logo_uri: Option<String>,
}

// ── Client ────────────────────────────────────────────────────────────

/// Client for the Helius Digital Asset Standard (DAS) API.
///
/// Owns its own `reqwest::Client`, separate from the Jupiter client
/// and from the indexer's RPC client.
#[derive(Clone)]
pub struct HeliusDasClient {
    http: reqwest::Client,
    /// Full Helius RPC URL, with API key — the same endpoint used for
    /// JSON-RPC calls (`getAssetBatch` is one of them).
    rpc_url: String,
}

impl HeliusDasClient {
    /// Build the client against the given Helius RPC URL.
    pub fn new(rpc_url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            rpc_url,
        }
    }

    /// Fetch metadata for a batch of mints.
    ///
    /// `mints` must not exceed `DAS_BATCH_MAX`; the worker chunks the
    /// queue before calling.
    ///
    /// Returns the subset of mints that yielded usable metadata
    /// (decimals present). Mints unknown to DAS or missing decimals
    /// are silently dropped.
    pub async fn fetch_asset_batch(
        &self,
        mints: &[Pubkey],
    ) -> Result<Vec<FetchedMetadata>, SourceError> {
        if mints.is_empty() {
            return Ok(Vec::new());
        }
        debug_assert!(mints.len() <= DAS_BATCH_MAX);

        // Pubkey -> base58 string at the HTTP boundary.
        let ids: Vec<String> = mints.iter().map(|m| m.to_string()).collect();

        let request = DasRequest {
            jsonrpc: "2.0",
            id: "yog-context",
            method: "getAssetBatch",
            params: DasParams { ids: &ids },
        };

        // Wrap transport failures in SourceError::Http — the worker
        // will log and retry on the next tick.
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
            .flatten() // drop nulls (mints unknown to DAS)
            .filter_map(into_fetched_metadata)
            .collect())
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

    // Symbol / name from metadata, if any.
    let (symbol, name) = asset
        .content
        .as_ref()
        .and_then(|c| c.metadata.as_ref())
        .map(|m| (m.symbol.clone(), m.name.clone()))
        .unwrap_or((None, None));

    // Logo: prefer `content.links.image`, fall back to first file.
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
    })
}
