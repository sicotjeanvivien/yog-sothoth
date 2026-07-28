use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

/// Pool properties that only exist for DAMM v2 — the per-protocol satellite of
/// the cross-protocol [`crate::domain::Pool`] registry.
///
/// # Why this is not on `Pool`
///
/// `Pool` records what every protocol has in common: an address, a protocol, a
/// token pair, a first and last sighting. The fields below are cp-amm concepts
/// with no DLMM (or Raydium, or Orca) equivalent. Keeping them on `Pool` would
/// mean NULL columns for entire protocols — the shape "voie 3" rejects for event
/// tables, applied here to pool properties (migration 036).
///
/// `fee_bps` deliberately stays on `Pool`: it is a normalized cross-protocol
/// notion and a read surface (fee-tier filter, `list_fee_tiers`).
///
/// # Why every field is optional
///
/// The two groups are filled by different mechanisms, at different times, and
/// fail independently:
///
/// - the **fee-split percents** are resolved as a unit by yog-context reading the
///   on-chain cp-amm `Pool` account, so all three are `None` together until that
///   resolution happens;
/// - the **fee shape** (`base_fee_kind`, `has_dynamic_fee`) is decoded from the
///   genesis `InitializePool` event's raw fee blob, so both stay `None` for any
///   pool whose genesis the indexer never saw — the common case, since a pool's
///   creation is only observable when we were already watching the protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDammV2PoolProperties {
    /// The pool these properties describe.
    pub pool_address: Pubkey,

    /// Meteora's cut of the trading fee, as a whole percent (0..=100), decoded
    /// from the on-chain `Pool` account. `None` until yog-context resolves it.
    pub protocol_fee_percent: Option<u8>,

    /// A partner's cut of the trading fee, as a whole percent (0..=100), decoded
    /// from the on-chain `Pool` account (often 0). `None` until resolved.
    pub partner_fee_percent: Option<u8>,

    /// A referrer's cut of the trading fee, as a whole percent (0..=100), decoded
    /// from the on-chain `Pool` account (only charged when a swap carries a
    /// referral account). `None` until resolved.
    pub referral_fee_percent: Option<u8>,

    /// How the base fee behaves over time, decoded from the genesis fee config
    /// (`InitializePool`): `constant`, `scheduler_linear`,
    /// `scheduler_exponential` or `rate_limiter` (see
    /// `amm::damm_v2::BaseFeeKind::as_str`). `None` until that event is seen, or
    /// if the fee blob failed to decode.
    pub base_fee_kind: Option<String>,

    /// Whether a volatility-based dynamic fee sits on top of the base fee,
    /// decoded from the same genesis config. Orthogonal to `base_fee_kind` — a
    /// pool can run a scheduler and a dynamic fee at once. `None` until decoded.
    pub has_dynamic_fee: Option<bool>,
}
