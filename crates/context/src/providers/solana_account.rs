//! Solana account source — fetches raw on-chain accounts by address.
//!
//! Pure transport, protocol-agnostic: it calls `getMultipleAccounts` (base64
//! encoding), base64-decodes each entry and hands the raw bytes plus the owner
//! to the caller. It knows nothing about pools, layouts or protocols.
//!
//! # Where the seam is
//!
//! base64 is the **RPC's** encoding, not the chain's, so it is decoded here and
//! never crosses into `core` — which stays free of transport concerns and of a
//! base64 dependency. Interpreting the bytes is
//! [`yog_core::application::decode_pool_account`]'s job: it routes on the
//! account's `owner`, which *is* its program id, so one client serves every
//! protocol.
//!
//! Accounts that do not exist are simply absent from the result — no error.
//! The caller retries them on the next cycle.

use async_trait::async_trait;
use base64::Engine;
use serde::Deserialize;
use solana_pubkey::Pubkey;

use super::metrics::ProviderMetrics;
use crate::error::SourceError;
use crate::source::{PoolAccountSource, RawAccount};
use std::time::Instant;

/// `getMultipleAccounts` accepts at most 100 keys per call.
const ACCOUNTS_BATCH_MAX: usize = 100;
const PROVIDER_LABEL: &str = "solana_account";

// ── Wire types ────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    id: &'static str,
    method: &'static str,
    params: (Vec<String>, RpcConfig<'a>),
}

#[derive(Debug, serde::Serialize)]
struct RpcConfig<'a> {
    encoding: &'a str,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: RpcResult,
}

#[derive(Debug, Deserialize)]
struct RpcResult {
    /// One entry per requested key, positionally. `null` when the
    /// account does not exist.
    value: Vec<Option<RpcAccount>>,
}

#[derive(Debug, Deserialize)]
struct RpcAccount {
    /// `[base64_data, "base64"]`.
    data: (String, String),
    owner: String,
}

#[derive(Clone)]
pub struct SolanaAccountClient {
    http: reqwest::Client,
    rpc_url: String,
}

impl SolanaAccountClient {
    pub fn new(rpc_url: String) -> Self {
        Self {
            http: super::http_client(),
            rpc_url,
        }
    }

    async fn fetch_chunk(&self, pools: &[Pubkey]) -> Result<Vec<RawAccount>, SourceError> {
        let start = Instant::now();
        let result = self.fetch_chunk_inner(pools).await;
        let outcome = match &result {
            Ok(_) => "ok",
            Err(SourceError::Http(_)) => "http",
            Err(SourceError::RateLimited { .. }) => "rate_limited",
            Err(SourceError::Decode(_)) => "decode",
        };
        ProviderMetrics::record_call(PROVIDER_LABEL, outcome, start.elapsed().as_secs_f64());
        result
    }

    async fn fetch_chunk_inner(&self, pools: &[Pubkey]) -> Result<Vec<RawAccount>, SourceError> {
        let keys: Vec<String> = pools.iter().map(|p| p.to_string()).collect();
        let request = RpcRequest {
            jsonrpc: "2.0",
            id: "yog-context",
            method: "getMultipleAccounts",
            params: (keys, RpcConfig { encoding: "base64" }),
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
            .json::<RpcResponse>()
            .await
            .map_err(|e| SourceError::Decode(e.to_string()))?;

        // Zip each requested address with its (possibly null) account. Missing
        // accounts and undecodable base64 are dropped — retried next cycle.
        Ok(pools
            .iter()
            .zip(response.result.value)
            .filter_map(|(address, account)| {
                let account = account?;
                let owner = Pubkey::try_from(account.owner.as_str()).ok()?;
                let data = base64::engine::general_purpose::STANDARD
                    .decode(account.data.0)
                    .ok()?;
                Some(RawAccount {
                    address: *address,
                    owner,
                    data,
                })
            })
            .collect())
    }
}

#[async_trait]
impl PoolAccountSource for SolanaAccountClient {
    async fn fetch_accounts(&self, pools: &[Pubkey]) -> Result<Vec<RawAccount>, SourceError> {
        let mut all = Vec::with_capacity(pools.len());
        for chunk in pools.chunks(ACCOUNTS_BATCH_MAX) {
            all.extend(self.fetch_chunk(chunk).await?);
        }
        Ok(all)
    }
}
