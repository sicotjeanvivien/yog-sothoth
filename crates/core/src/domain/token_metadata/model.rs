//! Token metadata domain model.
//!
//! The identity of an SPL mint — symbol, name, decimals, logo —
//! enriched from Helius DAS by the `yog-context` daemon. Pure domain
//! type: no persistence, no serialization concerns.

use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::CoreError;

/// Identity and display metadata for a single SPL mint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenMetadata {
    /// The SPL mint address.
    ///
    /// A `Pubkey`, consistent with `Pool` (`token_a_mint` /
    /// `token_b_mint`) — the domain types addresses strongly; the
    /// `Pubkey <-> TEXT` conversion happens in the persistence layer.
    pub mint: Pubkey,

    /// Token symbol (e.g. "USDC"). `None` when the mint carries no
    /// Metaplex metadata — rare, but valid.
    pub symbol: Option<String>,

    /// Token name (e.g. "USD Coin"). `None` for the same reason as
    /// `symbol`.
    pub name: Option<String>,

    /// Decimal precision of the mint. Always available from DAS — it
    /// is the field the indexer most needs to render raw amounts.
    pub decimals: u8,

    /// Logo URI as returned by DAS. May be an `ipfs://` URI — stored
    /// verbatim, resolved by the frontend.
    pub logo_uri: Option<String>,

    /// Which source produced this metadata (e.g. "helius_das").
    pub metadata_provider: MetadataProvider,

    /// When the metadata was first fetched.
    pub fetched_at: DateTime<Utc>,

    /// When the metadata was last refreshed.
    pub last_refresh_at: DateTime<Utc>,
}

/// Origin of a price observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataProvider {
    /// Fetched from Helius (DAS `price_info`).
    HeliusDas,
}

impl MetadataProvider {
    /// Stable lowercase tag, as persisted in the `price_source`
    /// column.
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
