use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::MeteoraDlmmPoolProperties;

/// DLMM-only pool properties (baseline §9's satellite table).
///
/// Every field is optional together, not independently: all six come from one
/// read of one `LbPair` account, so they are present for a resolved pool and
/// absent for one yog-context has not reached.
///
/// The pool's *state* — the active bin, the volatility accumulator — is not
/// here. It moves on every swap and belongs to the current-state surface.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDlmmPropertiesResponse {
    /// Price increment between adjacent bins, in basis points: bin `i` sits at
    /// `(1 + binStep / 10_000)^i`. The defining property of a DLMM pool.
    pub(super) bin_step: Option<u16>,
    /// The two other inputs to the pool's base fee, served raw so a client can
    /// recompute `feeBps` rather than trust it: `baseFactor × binStep ×
    /// 10^baseFeePowerFactor / 10_000`.
    pub(super) base_factor: Option<u16>,
    pub(super) base_fee_power_factor: Option<u8>,
    /// Magnitude of the volatility-driven fee charged on top of the base fee.
    /// **Zero means no dynamic fee** — DLMM has no boolean flag, unlike DAMM
    /// v2's `hasDynamicFee`.
    pub(super) variable_fee_control: Option<u32>,
    /// Per-pool ceiling on the volatility accumulator, and so on how far the
    /// variable fee can climb.
    pub(super) max_volatility_accumulator: Option<u32>,
    /// Meteora's cut of the trading fee, in **basis points** — not the whole
    /// percent DAMM v2's `protocolFeePercent` uses. The two are not comparable
    /// without scaling.
    pub(super) protocol_share: Option<u16>,
}

impl From<MeteoraDlmmPoolProperties> for MeteoraDlmmPropertiesResponse {
    fn from(p: MeteoraDlmmPoolProperties) -> Self {
        Self {
            bin_step: p.bin_step,
            base_factor: p.base_factor,
            base_fee_power_factor: p.base_fee_power_factor,
            variable_fee_control: p.variable_fee_control,
            max_volatility_accumulator: p.max_volatility_accumulator,
            protocol_share: p.protocol_share,
        }
    }
}
