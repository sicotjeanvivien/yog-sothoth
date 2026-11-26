use serde::{Deserialize, Serialize};

/// Direction of a swap in the pool's own A/B order: `AtoB`, the trader sent
/// token A and received token B; `BtoA` the reverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeDirection {
    AtoB,
    BtoA,
}

impl TradeDirection {
    /// Stable tag of the DB and the logs.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AtoB => "a_to_b",
            Self::BtoA => "b_to_a",
        }
    }

    /// Decode the Anchor event's `u8`: `0 = AtoB`, `1 = BtoA`, any other refused.
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
