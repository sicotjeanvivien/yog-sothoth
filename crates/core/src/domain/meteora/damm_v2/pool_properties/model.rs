use rust_decimal::Decimal;
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

/// Everything one read of a cp-amm `Pool` account yields — the properties that
/// are not inferable from the event stream. Written as a unit by yog-context via
/// [`super::MeteoraDammV2PoolAccountResolver::set_pool_account`].
///
/// Lives here, next to [`MeteoraDammV2PoolProperties`], and **not** in the
/// cross-protocol `pool` module: the fee-split percents are cp-amm concepts and
/// the mints are read at cp-amm's byte offsets. A DLMM `LbPair` account has a
/// different layout and a different property set, so it gets its own type rather
/// than widening this one — which is what keeps [`crate::domain::Pool`] free of
/// one protocol's vocabulary.
///
/// # Not the same type as [`MeteoraDammV2PoolProperties`]
///
/// This one is **total**: it is what a successful account read produces, so
/// every field is present. The other is **partial** (`Option` everywhere)
/// because it is what the database holds, filled by two independent writers at
/// different times. Merging them would force one of the two to lie.
///
/// The fields straddle two tables: `token_a_mint` / `token_b_mint` / `fee_bps`
/// land on the neutral `pools` registry, the three percents on this protocol's
/// satellite. The resolver writes both from this one value, atomically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteoraDammV2PoolAccountProperties {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    /// Base trading fee in basis points (genesis cliff for a scheduler pool).
    pub fee_bps: Decimal,
    /// Fee-split percents (0..=100): Meteora's, a partner's, and a referrer's
    /// cut of the trading fee.
    pub protocol_fee_percent: u8,
    pub partner_fee_percent: u8,
    pub referral_fee_percent: u8,
}
