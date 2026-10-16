use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

/// DLMM-only pool configuration, the satellite of [`crate::domain::Pool`]; the
/// fields are `None` together until yog-context resolves the account.
/// ⚠️ Configuration only: per-swap state here would rewrite the row on every bin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDlmmPoolProperties {
    /// The pool these properties describe.
    pub pool_address: Pubkey,

    /// Price increment between adjacent bins, in bps: bin `i` sits at
    /// `(1 + bin_step / 10_000)^i`.
    pub bin_step: Option<u16>,

    /// Base-fee multiplier, kept raw so `pools.fee_bps` stays recomputable.
    pub base_factor: Option<u16>,

    /// Power-of-ten scaling on the base fee, usually 0.
    pub base_fee_power_factor: Option<u8>,

    /// Magnitude of the volatility-driven fee; **zero means no dynamic fee**.
    pub variable_fee_control: Option<u32>,

    /// Ceiling on the volatility accumulator, so on the variable fee of this pool.
    pub max_volatility_accumulator: Option<u32>,

    /// Meteora's cut of the trading fee, in **basis points** (cp-amm: whole percent).
    pub protocol_share: Option<u16>,
}

/// What one read of an `LbPair` account yields for this protocol's satellite.
/// Total where [`MeteoraDlmmPoolProperties`] is partial: a read, not a row.
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
