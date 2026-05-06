use crate::CoreError;
use serde::{Deserialize, Serialize};
use solana_pubkey::{Pubkey, pubkey};

/// Supported AMM protocols.
///
/// Used to route incoming transactions to the correct protocol parser and
/// to identify the protocol in stored events and metrics.
///
/// String representations (used in SQL and JSON) are the fully qualified
/// snake_case variant names: `"meteora_damm_v2"`, `"meteora_damm_v1"`, `"meteora_dlmm"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    MeteoraDammV2,
    MeteoraDammV1,
    MeteoraDlmm,
}

const METEORA_DAMM_V2_PROGRAM_ID: Pubkey = pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");
const METEORA_DAMM_V1_PROGRAM_ID: Pubkey = pubkey!("Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB");
const METEORA_DLMM_PROGRAM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

impl Protocol {
    /// Returns the on-chain Program ID for this protocol on Solana mainnet,
    /// Validated at compile time via the `pubkey!` macro.
    pub fn program_id(&self) -> Pubkey {
        match self {
            Protocol::MeteoraDammV2 => METEORA_DAMM_V2_PROGRAM_ID,
            Protocol::MeteoraDammV1 => METEORA_DAMM_V1_PROGRAM_ID,
            Protocol::MeteoraDlmm => METEORA_DLMM_PROGRAM_ID,
        }
    }

    /// Returns the canonical snake_case string representation.
    /// Used for SQL INSERTs and log output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::MeteoraDammV2 => "meteora_damm_v2",
            Protocol::MeteoraDammV1 => "meteora_damm_v1",
            Protocol::MeteoraDlmm => "meteora_dlmm",
        }
    }

    /// Returns all supported protocols. Useful at startup to register every
    /// protocol the listener should subscribe to.
    pub fn all() -> &'static [Protocol] {
        &[
            Protocol::MeteoraDammV2,
            Protocol::MeteoraDammV1,
            Protocol::MeteoraDlmm,
        ]
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
            "meteora_damm_v2" => Ok(Protocol::MeteoraDammV2),
            "meteora_damm_v1" => Ok(Protocol::MeteoraDammV1),
            "meteora_dlmm" => Ok(Protocol::MeteoraDlmm),
            _ => Err(CoreError::UnknownProgram(s.to_string())),
        }
    }
}
