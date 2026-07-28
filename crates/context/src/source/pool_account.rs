use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::error::SourceError;

/// A raw on-chain account, as fetched — not interpreted.
///
/// `owner` is the program that owns the account, which on Solana *is* its
/// program id; it is what [`yog_core::application::decode_pool_account`] routes
/// on to pick a layout. `data` is already base64-decoded — base64 is the RPC's
/// encoding, not the chain's, so it stops at this boundary and never reaches
/// `core`.
#[derive(Debug, Clone)]
pub(crate) struct RawAccount {
    pub(crate) address: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) data: Vec<u8>,
}

/// Abstraction over a source of raw on-chain accounts.
///
/// Implemented by `SolanaAccountClient`. Behind a trait so the resolver worker
/// can be unit-tested against a fake source.
///
/// **Protocol-agnostic on purpose.** It used to return decoded cp-amm
/// properties, which made every consumer cp-amm-specific by construction — the
/// worker included. It now returns bytes: one client serves every protocol, and
/// the layouts live in `core`.
#[async_trait]
pub trait PoolAccountSource: Send + Sync {
    /// Fetch the accounts for a batch of addresses.
    ///
    /// Addresses the source cannot fetch (the account does not exist, the entry
    /// is malformed) are silently absent from the result — they'll be retried on
    /// the next poll cycle. Only a hard transport failure is an error.
    async fn fetch_accounts(&self, addresses: &[Pubkey]) -> Result<Vec<RawAccount>, SourceError>;
}
