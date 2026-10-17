/// What one read of an `LbPair` account yields for this protocol's satellite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeteoraDlmmPoolAccountProperties {
    /// Price increment between adjacent bins, in basis points.
    pub bin_step: u16,
    /// Base-fee multiplier, with its power-of-ten scaling.
    pub base_factor: u16,
    pub base_fee_power_factor: u8,
    /// Dynamic-fee magnitude (`0`: none), then its per-pool ceiling.
    pub variable_fee_control: u32,
    pub max_volatility_accumulator: u32,
    /// Meteora's cut of the trading fee, in basis points.
    pub protocol_share: u16,
}
