use crate::CoreError;
use serde::{Deserialize, Serialize};
use solana_pubkey::{pubkey, Pubkey};

/// Supported AMM protocols.
///
/// Used to route a watched pool to the correct [`crate::protocols::PoolIndexer`]
/// implementation, and to identify the protocol in stored metrics.
///
/// String representations (used in SQL and JSON) match the `snake_case` variant
/// names: `"damm_v2"`, `"damm_v1"`, `"dlmm"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    DammV2,
    DammV1,
    Dlmm,
}

const DAMM_V2_PROGRAM_ID: Pubkey = pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");
const DAMM_V1_PROGRAM_ID: Pubkey = pubkey!("Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB");
const DLMM_PROGRAM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

impl Protocol {
    /// Returns the on-chain Program ID for this protocol on Solana mainnet,
    /// Validated at compile time via the `pubkey!` macro.
    pub fn program_id(&self) -> Pubkey {
        match self {
            Protocol::DammV2 => DAMM_V2_PROGRAM_ID,
            Protocol::DammV1 => DAMM_V1_PROGRAM_ID,
            Protocol::Dlmm => DLMM_PROGRAM_ID,
        }
    }

    /// Returns the canonical snake_case string representation.
    /// Used for SQL INSERTs and log output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::DammV2 => "damm_v2",
            Protocol::DammV1 => "damm_v1",
            Protocol::Dlmm => "dlmm",
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Protocol {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "damm_v2" => Ok(Protocol::DammV2),
            "damm_v1" => Ok(Protocol::DammV1),
            "dlmm" => Ok(Protocol::Dlmm),
            _ => Err(CoreError::UnknownProgram(s.to_string())),
        }
    }
}
