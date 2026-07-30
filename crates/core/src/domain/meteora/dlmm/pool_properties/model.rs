use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

/// Pool properties that only exist for DLMM — the per-protocol satellite of the
/// cross-protocol [`crate::domain::Pool`] registry (migration 039).
///
/// # Why this is not on `Pool`
///
/// Same reason as [`crate::domain::MeteoraDammV2PoolProperties`]: `Pool` records
/// what every protocol has in common, and the fields below are Liquidity Book
/// concepts with no cp-amm equivalent. There is no `bin_step` in a
/// constant-product pool, and no fee scheduler in a bin-based one.
///
/// `fee_bps` deliberately stays on `Pool`. DLMM's base fee is a *floor* on what
/// a swapper pays, exactly like cp-amm's cliff fee numerator — the same notion,
/// so the same normalized column, which is what puts DLMM in the fee-tier filter
/// and `list_fee_tiers`. See [`crate::amm::dlmm::base_fee_bps`].
///
/// # Configuration, not state
///
/// Everything here changes only on an `update_fee_parameters`. The pool's *state*
/// — `active_id`, the volatility accumulator and its decay — moves on every swap
/// and belongs to `pool_current_state`, not to a satellite that would then be
/// rewritten on every crossed bin.
///
/// # Why every field is optional
///
/// Every field comes from the same source — yog-context reading the on-chain
/// `LbPair` account — so they are `None` **together**, for a pool that has been
/// discovered but not yet resolved. Unlike cp-amm's `base_fee_kind`, no field
/// here has a partial-failure mode: they are fixed-offset integers with no
/// enum to recognise, so a successful decode fills all of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteoraDlmmPoolProperties {
    /// The pool these properties describe.
    pub pool_address: Pubkey,

    /// Price increment between two adjacent bins, in basis points: bin `i` sits
    /// at `(1 + bin_step / 10_000)^i`. The defining property of a DLMM pool —
    /// the analogue of a fee tier — and one of the two inputs to its base fee.
    /// `None` until yog-context resolves it.
    pub bin_step: Option<u16>,

    /// Base-fee multiplier. With `bin_step` and `base_fee_power_factor` it
    /// yields the pool's base fee; kept raw so the derivation stays auditable
    /// and recomputable rather than only surviving as `pools.fee_bps`.
    pub base_factor: Option<u16>,

    /// Power-of-ten scaling on the base fee, for pools whose fee would not fit
    /// the `base_factor × bin_step` product alone. Usually 0.
    pub base_fee_power_factor: Option<u8>,

    /// Magnitude of the volatility-driven fee that sits on top of the base fee:
    /// `variable_fee_rate = ⌈variable_fee_control × (volatility_accumulator ×
    /// bin_step)² / 1e11⌉`. **Zero means no dynamic fee** — DLMM has no boolean
    /// flag, unlike cp-amm's `has_dynamic_fee`; the magnitude carries both facts.
    pub variable_fee_control: Option<u32>,

    /// Ceiling on the volatility accumulator, and so on how far the variable fee
    /// can climb for this pool. The per-pool bound below the chain-wide 10 % cap.
    pub max_volatility_accumulator: Option<u32>,

    /// Meteora's cut of the trading fee. The DLMM analogue of cp-amm's
    /// `protocol_fee_percent` — but in **basis points**, not whole percent.
    pub protocol_share: Option<u16>,
}

/// The DLMM-only properties one read of an `LbPair` account yields. Written by
/// yog-context via [`crate::domain::PoolAccountResolver::set_pool_account`],
/// wrapped in the [`crate::domain::PoolAccountProperties`] enum.
///
/// # Not the same type as [`MeteoraDlmmPoolProperties`]
///
/// This one is **total**: it is what a successful account read produces. The
/// other is **partial** (`Option` everywhere) because it is what the database
/// holds, which includes rows nobody has resolved yet. Merging them would force
/// one of the two to lie — the same split cp-amm makes, for the same reason.
///
/// Every field here lands on **this protocol's satellite and nowhere else**. The
/// neutral half of the same account read — the mints and the base fee, which
/// every protocol has — travels as [`crate::domain::PoolRegistryProperties`] and
/// is written by the `pools` registry's own repository.
///
/// # No field is optional, and none can be
///
/// cp-amm's equivalent carries one `Option` — `base_fee_kind`, whose
/// `BaseFeeMode` byte the program can extend with a value this build cannot map.
/// The `LbPair` layout has no such byte: every field below is a plain integer at
/// a fixed offset, so a decode that passes the discriminator and length checks
/// produces all of them or none. There is nothing to degrade gracefully, and so
/// no equivalent of cp-amm's "an unknown mode costs this one field" carve-out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeteoraDlmmPoolAccountProperties {
    /// Price increment between adjacent bins, in basis points.
    pub bin_step: u16,
    /// Base-fee multiplier, with its power-of-ten scaling.
    pub base_factor: u16,
    pub base_fee_power_factor: u8,
    /// Dynamic-fee parameters: magnitude, then per-pool ceiling. `0` magnitude
    /// means the pool charges no variable fee at all.
    pub variable_fee_control: u32,
    pub max_volatility_accumulator: u32,
    /// Meteora's cut of the trading fee, in basis points.
    pub protocol_share: u16,
}
