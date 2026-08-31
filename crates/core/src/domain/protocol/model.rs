use crate::CoreError;
use serde::{Deserialize, Serialize};
use solana_pubkey::{Pubkey, pubkey};

/// Supported AMM protocols.
///
/// Used to route incoming transactions to the correct protocol parser and
/// to identify the protocol in stored events and metrics.
///
/// String representations (used in SQL and JSON) are the fully qualified
/// snake_case variant names: `"meteora_damm_v2"`, `"meteora_dlmm"`.
///
/// A variant means "a protocol this project indexes", not "a protocol Meteora
/// ships". DAMM v1 was carried here as an empty placeholder until 31 August
/// 2026 and removed: it had no extractor, no decoder, no subscription and no
/// row in any table, so every arm mentioning it existed only to say "not this
/// one". Adding it back is the add-a-protocol recipe, not an edit here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    MeteoraDammV2,
    MeteoraDlmm,
}

const METEORA_DAMM_V2_PROGRAM_ID: Pubkey = pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");
const METEORA_DLMM_PROGRAM_ID: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

impl Protocol {
    /// Returns the on-chain Program ID for this protocol on Solana mainnet,
    /// Validated at compile time via the `pubkey!` macro.
    pub fn program_id(&self) -> Pubkey {
        match self {
            Protocol::MeteoraDammV2 => METEORA_DAMM_V2_PROGRAM_ID,
            Protocol::MeteoraDlmm => METEORA_DLMM_PROGRAM_ID,
        }
    }

    /// The protocol a program id belongs to, or `None` for a program we do not
    /// index.
    ///
    /// The reverse of [`Self::program_id`]. On Solana an account's `owner` *is*
    /// the program that owns it, so this is what lets a decoder route a raw
    /// account to the right per-protocol layout without the caller having to
    /// know which protocol it asked for.
    ///
    /// Returning `None` rather than erroring is deliberate: an account owned by
    /// an unknown program is not a failure, it is simply not ours.
    pub fn from_program_id(program_id: &Pubkey) -> Option<Protocol> {
        Self::all()
            .iter()
            .copied()
            .find(|protocol| &protocol.program_id() == program_id)
    }

    /// Returns the canonical snake_case string representation.
    /// Used for SQL INSERTs and log output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::MeteoraDammV2 => "meteora_damm_v2",
            Protocol::MeteoraDlmm => "meteora_dlmm",
        }
    }

    /// Returns all supported protocols. Useful at startup to register every
    /// protocol the listener should subscribe to.
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
