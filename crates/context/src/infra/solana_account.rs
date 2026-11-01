//! Solana account source: `getMultipleAccounts` in base64, decoded here so the RPC's encoding never
//! reaches `core`, which routes the raw bytes on their owner. A missing account is simply absent,
//! retried next cycle.

use async_trait::async_trait;
use base64::Engine;
use serde::Deserialize;
use solana_pubkey::Pubkey;

use yog_bootstrap::SecretUrl;

use super::metrics::ProviderMetrics;
use crate::application::source::{PoolAccountSource, RawAccount};
use crate::error::SourceError;
use std::time::Instant;

/// `getMultipleAccounts` accepts at most 100 keys per call.
const ACCOUNTS_BATCH_MAX: usize = 100;
const PROVIDER_LABEL: &str = "solana_account";

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
    /// One entry per requested key, in order; `null` when the account does not exist.
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
    /// Wrapped like [`super::helius_das::HeliusDasClient`]'s.
    rpc_url: SecretUrl,
}

impl SolanaAccountClient {
    pub fn new(rpc_url: SecretUrl) -> Self {
        Self {
            http: super::http_client(),
            rpc_url,
        }
    }

    async fn fetch_chunk(&self, pool_addresses: &[Pubkey]) -> Result<Vec<RawAccount>, SourceError> {
        let start = Instant::now();
        let result = self.fetch_chunk_inner(pool_addresses).await;
        let outcome = match &result {
            Ok(_) => "ok",
            Err(SourceError::Http(_)) => "http",
            Err(SourceError::RateLimited { .. }) => "rate_limited",
            Err(SourceError::Decode(_)) => "decode",
        };
        ProviderMetrics::record_call(PROVIDER_LABEL, outcome, start.elapsed().as_secs_f64());
        result
    }

    async fn fetch_chunk_inner(
        &self,
        pool_addresses: &[Pubkey],
    ) -> Result<Vec<RawAccount>, SourceError> {
        let keys: Vec<String> = pool_addresses.iter().map(|p| p.to_string()).collect();
        let request = RpcRequest {
            jsonrpc: "2.0",
            id: "yog-context",
            method: "getMultipleAccounts",
            params: (keys, RpcConfig { encoding: "base64" }),
        };

        let response = self
            .http
            .post(self.rpc_url.expose())
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<RpcResponse>()
            .await?;

        // Missing accounts and undecodable base64 are dropped, retried next cycle.
        Ok(pool_addresses
            .iter()
            .zip(response.result.value)
            .filter_map(|(pool_address, account)| {
                let account = account?;
                // `owner` on the wire is our `program_id`: same pubkey.
                let program_id = Pubkey::try_from(account.owner.as_str()).ok()?;
                let data = base64::engine::general_purpose::STANDARD
                    .decode(account.data.0)
                    .ok()?;
                Some(RawAccount {
                    pool_address: *pool_address,
                    program_id,
                    data,
                })
            })
            .collect())
    }
}

#[async_trait]
impl PoolAccountSource for SolanaAccountClient {
    async fn fetch_accounts(
        &self,
        pool_addresses: &[Pubkey],
    ) -> Result<Vec<RawAccount>, SourceError> {
        let mut all = Vec::with_capacity(pool_addresses.len());
        for chunk in pool_addresses.chunks(ACCOUNTS_BATCH_MAX) {
            all.extend(self.fetch_chunk(chunk).await?);
        }
        Ok(all)
    }
}

#[cfg(test)]
#[path = "solana_account_tests.rs"]
mod tests;
