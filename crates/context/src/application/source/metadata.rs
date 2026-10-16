use async_trait::async_trait;
use solana_pubkey::Pubkey;
use yog_core::domain::MetadataProvider;

use crate::error::SourceError;

/// A fetched piece of metadata; the worker adds the timestamps and the `metadata_source` tag.
#[derive(Debug, Clone)]
pub(crate) struct FetchedMetadata {
    pub(crate) mint: Pubkey,
    pub(crate) symbol: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) decimals: u8,
    pub(crate) logo_uri: Option<String>,
    pub(crate) metadata_provider: MetadataProvider,
}

/// Abstraction over a source of token metadata, implemented by `HeliusDasClient`.
#[async_trait]
pub trait MetadataSource: Send + Sync {
    /// Fetch metadata for a batch of mints, within the source's own batch limit; a mint unknown
    /// or lacking data is dropped from the result.
    async fn fetch_metadata(&self, mints: &[Pubkey]) -> Result<Vec<FetchedMetadata>, SourceError>;
}
