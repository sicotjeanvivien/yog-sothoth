use async_trait::async_trait;
use solana_pubkey::Pubkey;
use yog_core::domain::MetadataProvider;

use crate::error::SourceError;

/// A successfully fetched piece of metadata, ready to be turned into
/// the domain `TokenMetadata` by the worker.
///
/// This is the source-layer view: it carries the bits the worker
/// needs, but does NOT carry timestamps or the `metadata_source` tag
/// — those are added by the worker when building the domain object.
#[derive(Debug, Clone)]
pub(crate) struct FetchedMetadata {
    pub(crate) mint: Pubkey,
    pub(crate) symbol: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) decimals: u8,
    pub(crate) logo_uri: Option<String>,
    pub(crate) metadata_provider: MetadataProvider,
}

/// Abstraction over a source of token metadata.
///
/// Currently implemented by `HeliusDasClient`. Behind a trait so the
/// `MetadataWorker` can be unit-tested against a fake source.
#[async_trait]
pub trait MetadataSource: Send + Sync {
    /// Fetch metadata for a batch of mints.
    ///
    /// Implementations must respect their own batch limit (the worker
    /// chunks the queue before calling). The returned `Vec` contains
    /// only the mints the source successfully resolved — mints
    /// unknown to the source, or lacking enough data to be projected
    /// to `FetchedMetadata`, are silently dropped.
    async fn fetch_metadata(&self, mints: &[Pubkey]) -> Result<Vec<FetchedMetadata>, SourceError>;
}
