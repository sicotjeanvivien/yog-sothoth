//! cp-amm pool account source — resolves a pool's token mints.
//!
//! Calls the `getMultipleAccounts` JSON-RPC method (base64 encoding) and
//! decodes the two mints straight out of the on-chain cp-amm `Pool`
//! account layout. This is the authoritative source of a pool's mints,
//! replacing the per-event transferChecked heuristic that mis-resolved
//! routed/multi-hop transactions.
//!
//! # Pool account layout (cp-amm)
//!
//! 8-byte Anchor discriminator, then fixed-offset fields. Empirically
//! verified against mainnet (and stable across the program's ABI):
//!
//! - `cliff_fee_numerator` (base fee) — the leading `u64` at byte offset 8
//!   (`pool_fees` is the first field; its base fee numerator leads it)
//! - `token_a_mint` at byte offset 168 (32 bytes)
//! - `token_b_mint` at byte offset 200 (32 bytes)
//!
//! The account is owned by the cp-amm program and is 1112 bytes long.

use async_trait::async_trait;
use base64::Engine;
use serde::Deserialize;
use solana_pubkey::Pubkey;

use yog_core::domain::Protocol;

use super::metrics::ProviderMetrics;
use crate::error::SourceError;
use crate::source::{PoolAccountSource, ResolvedPoolAccount};
use std::time::Instant;

/// `getMultipleAccounts` accepts at most 100 keys per call.
const ACCOUNTS_BATCH_MAX: usize = 100;
const PROVIDER_LABEL: &str = "cpamm_pool";

/// Anchor account discriminator for the cp-amm `Pool` account
/// (`sha256("account:Pool")[..8]`) — guards against decoding the wrong
/// account shape.
const POOL_DISCRIMINATOR: [u8; 8] = [0xf1, 0x9a, 0x6d, 0x04, 0x11, 0xb1, 0x6d, 0xbc];
/// `cliff_fee_numerator`: the leading `u64` of `pool_fees`, right after the
/// 8-byte discriminator. The same quantity decoded from the genesis event,
/// validated against mainnet pool accounts.
const CLIFF_FEE_NUMERATOR_OFFSET: usize = 8;
const TOKEN_A_MINT_OFFSET: usize = 168;
const TOKEN_B_MINT_OFFSET: usize = 200;

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
pub struct CpAmmPoolClient {
    http: reqwest::Client,
    rpc_url: String,
    program_id: String,
}

impl CpAmmPoolClient {
    pub fn new(rpc_url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            rpc_url,
            program_id: Protocol::MeteoraDammV2.program_id().to_string(),
        }
    }

    async fn fetch_chunk(&self, pools: &[Pubkey]) -> Result<Vec<ResolvedPoolAccount>, SourceError> {
        let start = Instant::now();
        let result = self.fetch_chunk_inner(pools).await;
        let outcome = match &result {
            Ok(_) => "ok",
            Err(SourceError::Http(_)) => "http",
            Err(SourceError::Decode(_)) => "decode",
        };
        ProviderMetrics::record_call(PROVIDER_LABEL, outcome, start.elapsed().as_secs_f64());
        result
    }

    async fn fetch_chunk_inner(
        &self,
        pools: &[Pubkey],
    ) -> Result<Vec<ResolvedPoolAccount>, SourceError> {
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

        // Zip each requested pool with its (possibly null) account and
        // decode. Undecodable entries are dropped — retried next cycle.
        Ok(pools
            .iter()
            .zip(response.result.value)
            .filter_map(|(pool, account)| self.decode(*pool, account?))
            .collect())
    }

    /// Decode the mints and base fee from a pool account, or `None` if the
    /// owner, discriminator or length don't match the cp-amm `Pool` shape.
    fn decode(&self, pool: Pubkey, account: RpcAccount) -> Option<ResolvedPoolAccount> {
        if account.owner != self.program_id {
            return None;
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(account.data.0)
            .ok()?;
        if bytes.len() < TOKEN_B_MINT_OFFSET + 32 || bytes[..8] != POOL_DISCRIMINATOR {
            return None;
        }
        let cliff_fee_numerator = u64::from_le_bytes(
            bytes[CLIFF_FEE_NUMERATOR_OFFSET..CLIFF_FEE_NUMERATOR_OFFSET + 8]
                .try_into()
                .ok()?,
        );
        let token_a_mint =
            Pubkey::try_from(&bytes[TOKEN_A_MINT_OFFSET..TOKEN_A_MINT_OFFSET + 32]).ok()?;
        let token_b_mint =
            Pubkey::try_from(&bytes[TOKEN_B_MINT_OFFSET..TOKEN_B_MINT_OFFSET + 32]).ok()?;
        Some(ResolvedPoolAccount {
            pool,
            token_a_mint,
            token_b_mint,
            fee_bps: yog_core::amm::damm_v2::fee_numerator_to_bps(cliff_fee_numerator),
        })
    }
}

#[async_trait]
impl PoolAccountSource for CpAmmPoolClient {
    async fn fetch_accounts(
        &self,
        pools: &[Pubkey],
    ) -> Result<Vec<ResolvedPoolAccount>, SourceError> {
        let mut all = Vec::with_capacity(pools.len());
        for chunk in pools.chunks(ACCOUNTS_BATCH_MAX) {
            all.extend(self.fetch_chunk(chunk).await?);
        }
        Ok(all)
    }
}

#[cfg(test)]
#[path = "cpamm_pool_tests.rs"]
mod tests;
