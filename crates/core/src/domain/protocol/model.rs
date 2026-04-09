// core/src/domain/protocol.rs

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Supported AMM protocols.
///
/// Used to route a watched pool to the correct [`crate::protocols::PoolIndexer`]
/// implementation, and to identify the protocol in stored metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    DammV2,
    DammV1,
    Dlmm,
}

impl Protocol {
    /// Returns the canonical Program ID for this protocol on Solana mainnet.
    pub fn program_id(&self) -> Pubkey {
        match self {
            Protocol::DammV2 => "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG"
                .parse()
                .expect("hardcoded pubkey"),
            Protocol::DammV1 => todo!("DAMM v1 program ID"),
            Protocol::Dlmm => "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo"
                .parse()
                .expect("hardcoded pubkey"),
        }
    }

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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "damm_v2" => Ok(Protocol::DammV2),
            "damm_v1" => Ok(Protocol::DammV1),
            "dlmm" => Ok(Protocol::Dlmm),
            _ => Err(()),
        }
    }
}
