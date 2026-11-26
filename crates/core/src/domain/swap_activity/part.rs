use crate::domain::MeteoraDammV2LpFeeSplit;

/// The part of a swaps row only one protocol has, the variant naming that protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwapActivityPart {
    MeteoraDammV2(MeteoraDammV2LpFeeSplit),
}
