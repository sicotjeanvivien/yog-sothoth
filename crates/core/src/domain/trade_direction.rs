use serde::{Deserialize, Serialize};

/// Direction of a swap, relative to the canonical (token_a, token_b) ordering
/// of the pool.
///
/// - [`TradeDirection::AtoB`]: the trader provided token_a, received token_b
/// - [`TradeDirection::BtoA`]: the trader provided token_b, received token_a
///
/// Combined with the canonical mint ordering, this is enough to recover what
/// the trader sent and received from the `amount_a` / `amount_b` fields of a
/// [`crate::domain::SwapEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeDirection {
    AtoB,
    BtoA,
}

impl TradeDirection {
    /// Stable string representation, used for DB persistence and logs.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AtoB => "a_to_b",
            Self::BtoA => "b_to_a",
        }
    }

    /// Decode from the on-chain `u8` representation used by Anchor events.
    /// `0 = AtoB`, `1 = BtoA`. Other values are rejected.
    pub fn from_u8(v: u8) -> Result<Self, u8> {
        match v {
            0 => Ok(Self::AtoB),
            1 => Ok(Self::BtoA),
            other => Err(other),
        }
    }
}

impl std::fmt::Display for TradeDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TradeDirection {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "a_to_b" => Ok(Self::AtoB),
            "b_to_a" => Ok(Self::BtoA),
            _ => Err(()),
        }
    }
}
