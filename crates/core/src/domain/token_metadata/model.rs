//! The identity of an SPL mint (symbol, name, decimals, logo), from Helius DAS.

use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::CoreError;

/// Identity and display metadata for a single SPL mint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenMetadata {
    /// The SPL mint address.
    pub mint: Pubkey,

    /// Token symbol; `None` when the mint carries no Metaplex metadata.
    pub symbol: Option<String>,

    /// Token name; `None` like `symbol`.
    pub name: Option<String>,

    /// Decimal precision of the mint.
    pub decimals: u8,

    /// Logo URI as DAS returns it, `ipfs://` included; the frontend resolves it.
    pub logo_uri: Option<String>,

    /// Which source produced this metadata (e.g. "helius_das").
    pub metadata_provider: MetadataProvider,

    /// When the metadata was first fetched.
    pub fetched_at: DateTime<Utc>,

    /// When the metadata was last refreshed.
    pub last_refresh_at: DateTime<Utc>,
}

/// Origin of a metadata row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataProvider {
    /// Fetched from Helius DAS.
    HeliusDas,
}

impl MetadataProvider {
    /// Stable tag of the `metadata_provider` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            MetadataProvider::HeliusDas => "helius_das",
        }
    }
}

impl std::str::FromStr for MetadataProvider {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "helius_das" => Ok(MetadataProvider::HeliusDas),
            _ => Err(CoreError::UnknownProgram(s.to_string())),
        }
    }
}
