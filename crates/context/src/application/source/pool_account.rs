use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::error::SourceError;

/// A raw on-chain pool account, as fetched: `program_id` is its owner, what
/// [`yog_core::application::decode_pool_account`] routes on; `data` is already base64-decoded.
#[derive(Debug, Clone)]
pub(crate) struct RawAccount {
    pub(crate) pool_address: Pubkey,
    pub(crate) program_id: Pubkey,
    pub(crate) data: Vec<u8>,
}

/// Abstraction over a source of raw on-chain accounts; it returns bytes, so one client serves
/// every protocol and the layouts stay in `core`.
#[async_trait]
pub trait PoolAccountSource: Send + Sync {
    /// Fetch the accounts for a batch of pool addresses; one that cannot be fetched is absent and
    /// retried next cycle. Only a hard transport failure is an error.
    async fn fetch_accounts(
        &self,
        pool_addresses: &[Pubkey],
    ) -> Result<Vec<RawAccount>, SourceError>;
}
