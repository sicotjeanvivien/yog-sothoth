use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::amm::damm_v2::BaseFeeKind;

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
/// Every field comes from the same source — yog-context reading the on-chain
/// cp-amm `Pool` account — so they are `None` **together**, for a pool that has
/// been discovered but not yet resolved. This is not a per-field failure mode:
/// it is the row of a pool the enrichment queue has not reached.
///
/// The one exception is `base_fee_kind`, which can be `None` on a row where
/// everything else is filled: a `BaseFeeMode` this build cannot map costs that
/// field alone (see [`MeteoraDammV2PoolAccountProperties::base_fee_kind`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDammV2PoolProperties {
    /// The pool these properties describe.
    pub pool_address: Pubkey,

    /// Meteora's cut of the trading fee, as a whole percent (0..=100), decoded
    /// from the on-chain `Pool` account. `None` until yog-context resolves it.
    pub protocol_fee_percent: Option<u8>,

    // NOTE: there is no partner fee. A `partner_fee_percent` field lived here
    // until migration 037; it decoded byte 49 of the account, which cp-amm
    // declares as `padding_0`. See that migration for the evidence.
    /// A referrer's cut of the trading fee, as a whole percent (0..=100), decoded
    /// from the on-chain `Pool` account (only charged when a swap carries a
    /// referral account). `None` until resolved.
    pub referral_fee_percent: Option<u8>,

    /// How the base fee behaves over time, read from the account's `BaseFeeMode`
    /// and period count: `constant`, `scheduler_linear`, `scheduler_exponential`,
    /// `rate_limiter`, `market_cap_scheduler_linear` or
    /// `market_cap_scheduler_exponential` (see `amm::damm_v2::BaseFeeKind`).
    /// `None` until resolved, or if the mode is one this build cannot map.
    pub base_fee_kind: Option<String>,

    /// Whether a volatility-based dynamic fee sits on top of the base fee, from
    /// the same account read. Orthogonal to `base_fee_kind` — a pool can run a
    /// scheduler and a dynamic fee at once. `None` until resolved.
    pub has_dynamic_fee: Option<bool>,
}

/// The cp-amm-only properties one read of a `Pool` account yields. Written by
/// yog-context via [`crate::domain::PoolAccountResolver::set_pool_account`],
/// wrapped in the [`crate::domain::PoolAccountProperties`] enum.
///
/// Lives here, next to [`MeteoraDammV2PoolProperties`], and **not** in the
/// cross-protocol `pool` module: the fee-split percents and the fee shape are
/// cp-amm concepts, read at cp-amm's byte offsets. A DLMM `LbPair` account has a
/// different layout and a different property set, so it gets its own type rather
/// than widening this one — which is what keeps [`crate::domain::Pool`] free of
/// one protocol's vocabulary.
///
/// # Not the same type as [`MeteoraDammV2PoolProperties`]
///
/// This one is (near-)**total**: it is what a successful account read produces.
/// The other is **partial** (`Option` everywhere) because it is what the
/// database holds, which includes rows nobody has resolved yet. Merging them
/// would force one of the two to lie.
///
/// Every field here lands on **this protocol's satellite and nowhere else**. The
/// neutral half of the same account read — the mints and the base fee, which
/// every protocol has — travels as [`crate::domain::PoolRegistryProperties`] and is
/// written by the `pools` registry's own repository. That separation is what
/// gives each table a single writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteoraDammV2PoolAccountProperties {
    /// Fee-split percents (0..=100): Meteora's cut and a referrer's cut of the
    /// trading fee. They are **not adjacent** in the account — `padding_0` sits
    /// between them, at the offset a `partner_fee_percent` field used to be read
    /// from (migration 037).
    pub protocol_fee_percent: u8,
    pub referral_fee_percent: u8,

    /// How the base fee behaves over time, from the account's `BaseFeeMode`
    /// discriminant and scheduler period count.
    ///
    /// **The only optional field of this otherwise-total type**, and
    /// deliberately so. cp-amm can gain a `BaseFeeMode` we do not know; refusing
    /// the whole account in that case would drop the mints and the fee tier with
    /// it, and — since a pool that never resolves never leaves
    /// [`crate::domain::PoolAccountResolver::list_unresolved`] — it would sit at
    /// the head of the queue forever, starving every pool behind it. An unknown
    /// mode therefore costs this one field and nothing else.
    pub base_fee_kind: Option<BaseFeeKind>,

    /// Whether a volatility-based dynamic fee is enabled, from
    /// `DynamicFeeStruct::initialized`. Always decodable — it is a flag byte at
    /// a fixed offset, with no mode to recognise and no tri-state to resolve.
    pub has_dynamic_fee: bool,
}
