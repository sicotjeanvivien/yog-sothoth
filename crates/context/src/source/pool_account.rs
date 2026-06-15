use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::error::SourceError;

/// A pool's token mints, decoded from its on-chain account. The
/// authoritative source of a pool's mints — unlike the per-event
/// transferChecked heuristic the indexer used to rely on.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedPoolMints {
    pub(crate) pool: Pubkey,
    pub(crate) token_a_mint: Pubkey,
    pub(crate) token_b_mint: Pubkey,
}

/// Abstraction over a source of on-chain pool account state.
///
/// Implemented by `CpAmmPoolClient`. Behind a trait so the resolver
/// worker can be unit-tested against a fake source.
#[async_trait]
pub trait PoolAccountSource: Send + Sync {
    /// Fetch and decode the token mints for a batch of pool addresses.
    /// Pools the source can't fetch or decode (unknown account, wrong
    /// owner, short data) are silently dropped — they'll be retried on
    /// the next poll cycle.
    async fn fetch_mints(&self, pools: &[Pubkey]) -> Result<Vec<ResolvedPoolMints>, SourceError>;
}
