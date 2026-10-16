use crate::CoreError;
use serde::{Deserialize, Serialize};
use solana_pubkey::{Pubkey, pubkey};

/// The protocols this project indexes, not every protocol Meteora ships;
/// tagged `"meteora_damm_v2"`, `"meteora_dlmm"` in SQL and JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    MeteoraDammV2,
    MeteoraDlmm,
}

const METEORA_DAMM_V2_PROGRAM_ID: Pubkey = pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");
const METEORA_DLMM_PROGRAM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

impl Protocol {
    /// The protocol's mainnet program id.
    pub fn program_id(&self) -> Pubkey {
        match self {
            Protocol::MeteoraDammV2 => METEORA_DAMM_V2_PROGRAM_ID,
            Protocol::MeteoraDlmm => METEORA_DLMM_PROGRAM_ID,
        }
    }

    /// The protocol a program id belongs to, `None` for a program we do not index:
    /// an account's `owner` routes it to its layout.
    pub fn from_program_id(program_id: &Pubkey) -> Option<Protocol> {
        Self::all()
            .iter()
            .copied()
            .find(|protocol| &protocol.program_id() == program_id)
    }

    /// The canonical snake_case tag.
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::MeteoraDammV2 => "meteora_damm_v2",
            Protocol::MeteoraDlmm => "meteora_dlmm",
        }
    }

    /// Every protocol the domain names; ingestion subscribes to the implemented ones only.
    pub fn all() -> &'static [Protocol] {
        &[Protocol::MeteoraDammV2, Protocol::MeteoraDlmm]
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
            "meteora_dlmm" => Ok(Protocol::MeteoraDlmm),
            _ => Err(CoreError::UnknownProgram(s.to_string())),
        }
    }
}
