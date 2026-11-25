use crate::domain::ProtocolTvlMeteoraDammV2;

/// The part of a TVL reading only one protocol has, the variant naming that protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolTvlPart {
    MeteoraDammV2(ProtocolTvlMeteoraDammV2),
}
