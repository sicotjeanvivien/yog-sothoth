use async_trait::async_trait;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::error::SourceError;

/// A pool's account-derived properties, decoded from its on-chain account:
/// both token mints and the base trading fee (basis points). The authoritative
/// source for all of them — the mints because the per-event transferChecked
/// heuristic mis-resolved them, the fee because the genesis event is invisible
/// for pools that predate the indexer.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedPoolAccount {
    pub(crate) pool: Pubkey,
    pub(crate) token_a_mint: Pubkey,
    pub(crate) token_b_mint: Pubkey,
    pub(crate) fee_bps: Decimal,
}

/// Abstraction over a source of on-chain pool account state.
///
/// Implemented by `CpAmmPoolClient`. Behind a trait so the resolver
/// worker can be unit-tested against a fake source.
#[async_trait]
pub trait PoolAccountSource: Send + Sync {
    /// Fetch and decode the account properties for a batch of pool addresses.
    /// Pools the source can't fetch or decode (unknown account, wrong
    /// owner, short data) are silently dropped — they'll be retried on
    /// the next poll cycle.
    async fn fetch_accounts(
        &self,
        pools: &[Pubkey],
    ) -> Result<Vec<ResolvedPoolAccount>, SourceError>;
}
