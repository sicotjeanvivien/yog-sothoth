//! Translate DAMM v2 wire events into protocol-agnostic domain events.
//!
//! Wire events ([`super::events::DammV2WireEvent`]) are byte-perfect mirrors
//! of cp-amm's on-chain Anchor events. Domain events
//! ([`crate::domain::DomainEvent`]) are protocol-agnostic representations
//! consumed by the indexer service.
//!
//! Token mints are NOT derived here: they are a property of the pool,
//! resolved authoritatively from the cp-amm Pool account by yog-context. Swap
//! and liquidity events therefore carry no mints — they reference the pool.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::{
    domain::{
        MeteoraDammV2ClaimPositionFeeEvent, MeteoraDammV2ClaimRewardEvent,
        MeteoraDammV2ClosePositionEvent, MeteoraDammV2CreatePositionEvent,
        MeteoraDammV2InitializePoolEvent, MeteoraDammV2LiquidityEvent,
        MeteoraDammV2LiquidityEventKind, MeteoraDammV2LockPositionEvent,
        MeteoraDammV2PermanentLockPositionEvent, MeteoraDammV2SetPoolStatusEvent,
        MeteoraDammV2SwapEvent, MeteoraDammV2UpdatePoolFeesEvent, TradeDirection,
    },
    error::TranslationError,
};

use super::events::{
    DammV2WireEvent, EvtClaimPositionFee, EvtClaimReward, EvtClosePosition, EvtCreatePosition,
    EvtInitializePool, EvtLiquidityChange, EvtLockPosition, EvtPermanentLockPosition,
    EvtSetPoolStatus, EvtSwap2, EvtUpdatePoolFees,
};

// ---------------------------------------------------------------------------
// Per-variant translators (option C)
// ---------------------------------------------------------------------------

/// Translate an [`EvtSwap2`] into a [`MeteoraDammV2SwapEvent`].
///
/// Returns `Err` only if `trade_direction` is invalid (out of range).
pub(super) fn translate_swap(
    wire: &EvtSwap2,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> Result<MeteoraDammV2SwapEvent, TranslationError> {
    let trade_direction = TradeDirection::from_u8(wire.trade_direction).map_err(|raw| {
        TranslationError::InvalidEnum {
            field: "trade_direction",
            value: raw,
        }
    })?;

    let fee_token_is_a =
        compute_fee_token_is_a(wire.collect_fee_mode, trade_direction).map_err(|raw| {
            TranslationError::InvalidEnum {
                field: "collect_fee_mode",
                value: raw,
            }
        })?;

    // EvtSwap2 reports input/output amounts in
    // `included_transfer_fee_amount_in` / `included_transfer_fee_amount_out`.
    // Map them onto (amount_a, amount_b) according to trade direction:
    //   AtoB → input is on a, output is on b
    //   BtoA → input is on b, output is on a
    let (amount_a, amount_b) = match trade_direction {
        TradeDirection::AtoB => (
            wire.included_transfer_fee_amount_in,
            wire.included_transfer_fee_amount_out,
        ),
        TradeDirection::BtoA => (
            wire.included_transfer_fee_amount_out,
            wire.included_transfer_fee_amount_in,
        ),
    };
    Ok(MeteoraDammV2SwapEvent {
        pool_address: wire.pool,
        signature,
        timestamp,

        trade_direction,
        amount_a,
        amount_b,

        reserve_a_after: wire.reserve_a_amount,
        reserve_b_after: wire.reserve_b_amount,
        next_sqrt_price: wire.swap_result.next_sqrt_price,

        claiming_fee: wire.swap_result.claiming_fee,
        protocol_fee: wire.swap_result.protocol_fee,
        compounding_fee: wire.swap_result.compounding_fee,
        referral_fee: wire.swap_result.referral_fee,
        fee_token_is_a,
    })
}

/// Translate an [`EvtLiquidityChange`] into a [`MeteoraDammV2LiquidityEvent`].
pub(super) fn translate_liquidity(
    wire: &EvtLiquidityChange,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> Result<MeteoraDammV2LiquidityEvent, TranslationError> {
    let liquidity_event_kind =
        MeteoraDammV2LiquidityEventKind::from_u8(wire.change_type).map_err(|raw| {
            TranslationError::InvalidEnum {
                field: "change_type",
                value: raw,
            }
        })?;

    Ok(MeteoraDammV2LiquidityEvent {
        pool_address: wire.pool,
        signature,
        timestamp,

        liquidity_event_kind,
        amount_a: wire.token_a_amount,
        amount_b: wire.token_b_amount,
        liquidity_delta: wire.liquidity_delta,

        reserve_a_after: wire.reserve_a_amount,
        reserve_b_after: wire.reserve_b_amount,

        position: wire.position,
        owner: wire.owner,
    })
}

/// Translate an [`EvtClaimPositionFee`] into a [`MeteoraDammV2ClaimPositionFeeEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_claim_position_fee(
    wire: &EvtClaimPositionFee,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2ClaimPositionFeeEvent {
    MeteoraDammV2ClaimPositionFeeEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        position: wire.position,
        owner: wire.owner,
        fee_a_claimed: wire.fee_a_claimed,
        fee_b_claimed: wire.fee_b_claimed,
    }
}

/// Translate an [`EvtClaimReward`] into a [`MeteoraDammV2ClaimRewardEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_claim_reward(
    wire: &EvtClaimReward,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2ClaimRewardEvent {
    MeteoraDammV2ClaimRewardEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        position: wire.position,
        owner: wire.owner,
        mint_reward: wire.mint_reward,
        reward_index: wire.reward_index,
        total_reward: wire.total_reward,
    }
}

/// Translate an [`EvtCreatePosition`] into a [`MeteoraDammV2CreatePositionEvent`].
///
/// This translation is infallible — the wire event is self-contained
/// (pool, owner, position, position NFT mint), so no transferChecked
/// context is required.
pub(super) fn translate_create_position(
    wire: &EvtCreatePosition,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2CreatePositionEvent {
    MeteoraDammV2CreatePositionEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        owner: wire.owner,
        position: wire.position,
        position_nft_mint: wire.position_nft_mint,
    }
}

/// Translate an [`EvtClosePosition`] into a [`MeteoraDammV2ClosePositionEvent`].
///
/// Infallible — self-contained wire event, no transferChecked context needed.
pub(super) fn translate_close_position(
    wire: &EvtClosePosition,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2ClosePositionEvent {
    MeteoraDammV2ClosePositionEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        owner: wire.owner,
        position: wire.position,
        position_nft_mint: wire.position_nft_mint,
    }
}

/// Translate an [`EvtLockPosition`] into a [`MeteoraDammV2LockPositionEvent`].
///
/// Infallible — every field maps directly, no enum or context to resolve.
pub(super) fn translate_lock_position(
    wire: &EvtLockPosition,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2LockPositionEvent {
    MeteoraDammV2LockPositionEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        position: wire.position,
        owner: wire.owner,
        vesting: wire.vesting,
        cliff_point: wire.cliff_point,
        period_frequency: wire.period_frequency,
        cliff_unlock_liquidity: wire.cliff_unlock_liquidity,
        liquidity_per_period: wire.liquidity_per_period,
        number_of_period: wire.number_of_period,
    }
}

/// Translate an [`EvtPermanentLockPosition`] into a
/// [`MeteoraDammV2PermanentLockPositionEvent`]. Infallible.
pub(super) fn translate_permanent_lock_position(
    wire: &EvtPermanentLockPosition,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2PermanentLockPositionEvent {
    MeteoraDammV2PermanentLockPositionEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        position: wire.position,
        lock_liquidity_amount: wire.lock_liquidity_amount,
        total_permanent_locked_liquidity: wire.total_permanent_locked_liquidity,
    }
}

/// Translate an [`EvtInitializePool`] into a [`MeteoraDammV2InitializePoolEvent`].
///
/// Self-contained — the wire event carries both mints, so no transferChecked
/// context is needed. The fee parameters are re-serialized to borsh and stored
/// raw (undecoded) under "voie C". `borsh::to_vec` into a `Vec` cannot fail in
/// practice (no I/O), so the `expect` is unreachable.
pub(super) fn translate_initialize_pool(
    wire: &EvtInitializePool,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2InitializePoolEvent {
    let pool_fees_raw =
        borsh::to_vec(&wire.pool_fees).expect("borsh serialize to Vec is infallible");

    MeteoraDammV2InitializePoolEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        token_a_mint: wire.token_a_mint,
        token_b_mint: wire.token_b_mint,
        creator: wire.creator,
        payer: wire.payer,
        alpha_vault: wire.alpha_vault,
        sqrt_min_price: wire.sqrt_min_price,
        sqrt_max_price: wire.sqrt_max_price,
        sqrt_price: wire.sqrt_price,
        liquidity: wire.liquidity,
        activation_type: wire.activation_type,
        activation_point: wire.activation_point,
        collect_fee_mode: wire.collect_fee_mode,
        pool_type: wire.pool_type,
        token_a_flag: wire.token_a_flag,
        token_b_flag: wire.token_b_flag,
        token_a_amount: wire.token_a_amount,
        token_b_amount: wire.token_b_amount,
        total_amount_a: wire.total_amount_a,
        total_amount_b: wire.total_amount_b,
        pool_fees_raw,
    }
}

/// Translate an [`EvtSetPoolStatus`] into a [`MeteoraDammV2SetPoolStatusEvent`].
/// Infallible.
pub(super) fn translate_set_pool_status(
    wire: &EvtSetPoolStatus,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2SetPoolStatusEvent {
    MeteoraDammV2SetPoolStatusEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        status: wire.status,
    }
}

/// Translate an [`EvtUpdatePoolFees`] into a [`MeteoraDammV2UpdatePoolFeesEvent`].
/// Infallible — the fee params are carried through as the raw, undecoded blob.
pub(super) fn translate_update_pool_fees(
    wire: &EvtUpdatePoolFees,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2UpdatePoolFeesEvent {
    MeteoraDammV2UpdatePoolFeesEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        operator: wire.operator,
        params_raw: wire.params_raw.clone(),
    }
}

// ---------------------------------------------------------------------------
// Compute fee_token_is_a
// ---------------------------------------------------------------------------

/// Determine whether the fee was charged on token A (`true`) or token B
/// (`false`), based on the on-chain `collect_fee_mode` and the swap's
/// `trade_direction`.
///
/// Mirrors `cp-amm::FeeMode::get_fee_mode` — see source comments in
/// `cp-amm/programs/cp-amm/src/state/fee.rs`. Updated alongside cp-amm
/// upgrades.
fn compute_fee_token_is_a(
    collect_fee_mode: u8,
    trade_direction: TradeDirection,
) -> Result<bool, u8> {
    // CollectFeeMode mapping (mirrors cp-amm enum):
    //   0 = BothToken    — fee on the OUT token
    //   1 = OnlyB        — fee always on token B
    //   2 = Compounding  — fee always on token B
    let fee_token_is_a = match (collect_fee_mode, trade_direction) {
        (0, TradeDirection::AtoB) => false, // out is B, fee on B
        (0, TradeDirection::BtoA) => true,  // out is A, fee on A
        (1, _) => false,                    // OnlyB → always B
        (2, _) => false,                    // Compounding → always B
        (other, _) => return Err(other),
    };
    Ok(fee_token_is_a)
}

// ---------------------------------------------------------------------------
// High-level dispatch
// ---------------------------------------------------------------------------

/// Translate a single wire event into a domain event.
pub(super) fn translate_wire_event(
    wire: &DammV2WireEvent,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> Result<crate::domain::DomainEvent, TranslationError> {
    use crate::domain::DomainEvent;
    use crate::domain::MeteoraDammV2Event;

    let damm_v2_event = match wire {
        DammV2WireEvent::Swap2(e) => {
            MeteoraDammV2Event::Swap(translate_swap(e, signature, timestamp)?)
        }
        DammV2WireEvent::LiquidityChange(e) => {
            MeteoraDammV2Event::Liquidity(translate_liquidity(e, signature, timestamp)?)
        }
        DammV2WireEvent::ClaimPositionFee(e) => MeteoraDammV2Event::ClaimPositionFee(
            translate_claim_position_fee(e, signature, timestamp),
        ),
        DammV2WireEvent::ClaimReward(e) => {
            MeteoraDammV2Event::ClaimReward(translate_claim_reward(e, signature, timestamp))
        }
        DammV2WireEvent::CreatePosition(e) => {
            MeteoraDammV2Event::CreatePosition(translate_create_position(e, signature, timestamp))
        }
        DammV2WireEvent::ClosePosition(e) => {
            MeteoraDammV2Event::ClosePosition(translate_close_position(e, signature, timestamp))
        }
        DammV2WireEvent::LockPosition(e) => {
            MeteoraDammV2Event::LockPosition(translate_lock_position(e, signature, timestamp))
        }
        DammV2WireEvent::PermanentLockPosition(e) => MeteoraDammV2Event::PermanentLockPosition(
            translate_permanent_lock_position(e, signature, timestamp),
        ),
        DammV2WireEvent::InitializePool(e) => {
            MeteoraDammV2Event::InitializePool(translate_initialize_pool(e, signature, timestamp))
        }
        DammV2WireEvent::SetPoolStatus(e) => {
            MeteoraDammV2Event::SetPoolStatus(translate_set_pool_status(e, signature, timestamp))
        }
        DammV2WireEvent::UpdatePoolFees(e) => {
            MeteoraDammV2Event::UpdatePoolFees(translate_update_pool_fees(e, signature, timestamp))
        }
    };

    Ok(DomainEvent::MeteoraDammV2(damm_v2_event))
}

// ---------------------------------------------------------------------------
// Translation unit tests
// ---------------------------------------------------------------------------
//
// Field-mapping guards for the ring-2 lifecycle events that have no on-chain
// fixture yet (close / lock / permanent-lock / set-pool-status). They build a
// wire event with a distinct sentinel per field and assert each lands in the
// right domain field — catching swaps/typos in the translator. They do NOT
// validate the borsh layout or the discriminator against the real program;
// that still needs a fixture.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::extraction::meteora::damm_v2::events::{
        EvtClosePosition, EvtLockPosition, EvtPermanentLockPosition, EvtSetPoolStatus,
    };
    use solana_pubkey::Pubkey;

    fn pk(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }
    fn sig() -> Signature {
        Signature::from([7u8; 64])
    }
    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).unwrap()
    }

    #[test]
    fn close_position_maps_every_field() {
        let wire = EvtClosePosition {
            pool: pk(1),
            owner: pk(2),
            position: pk(3),
            position_nft_mint: pk(4),
        };
        let d = translate_close_position(&wire, sig(), ts());
        assert_eq!(d.pool_address, pk(1));
        assert_eq!(d.owner, pk(2));
        assert_eq!(d.position, pk(3));
        assert_eq!(d.position_nft_mint, pk(4));
        assert_eq!(d.signature, sig());
        assert_eq!(d.timestamp, ts());
    }

    #[test]
    fn lock_position_maps_every_field() {
        let wire = EvtLockPosition {
            pool: pk(1),
            position: pk(2),
            owner: pk(3),
            vesting: pk(4),
            cliff_point: 100,
            period_frequency: 200,
            cliff_unlock_liquidity: 300,
            liquidity_per_period: 400,
            number_of_period: 5,
        };
        let d = translate_lock_position(&wire, sig(), ts());
        assert_eq!(d.pool_address, pk(1));
        assert_eq!(d.position, pk(2));
        assert_eq!(d.owner, pk(3));
        assert_eq!(d.vesting, pk(4));
        assert_eq!(d.cliff_point, 100);
        assert_eq!(d.period_frequency, 200);
        assert_eq!(d.cliff_unlock_liquidity, 300);
        assert_eq!(d.liquidity_per_period, 400);
        assert_eq!(d.number_of_period, 5);
    }

    #[test]
    fn permanent_lock_position_maps_every_field() {
        let wire = EvtPermanentLockPosition {
            pool: pk(1),
            position: pk(2),
            lock_liquidity_amount: 111,
            total_permanent_locked_liquidity: 222,
        };
        let d = translate_permanent_lock_position(&wire, sig(), ts());
        assert_eq!(d.pool_address, pk(1));
        assert_eq!(d.position, pk(2));
        assert_eq!(d.lock_liquidity_amount, 111);
        assert_eq!(d.total_permanent_locked_liquidity, 222);
    }

    #[test]
    fn set_pool_status_maps_every_field() {
        let wire = EvtSetPoolStatus {
            pool: pk(9),
            status: 1,
        };
        let d = translate_set_pool_status(&wire, sig(), ts());
        assert_eq!(d.pool_address, pk(9));
        assert_eq!(d.status, 1);
        assert_eq!(d.signature, sig());
        assert_eq!(d.timestamp, ts());
    }

    // ── ring-1 fee-side logic ───────────────────────────────────────────

    /// `compute_fee_token_is_a` mirrors cp-amm's FeeMode. A wrong branch here
    /// mislabels which token a swap's fee is denominated in — every combination
    /// is pinned, including the unknown-mode error path.
    #[test]
    fn compute_fee_token_is_a_covers_every_mode() {
        // BothToken (0): fee on the OUT token → A only when the trade is B→A.
        assert_eq!(compute_fee_token_is_a(0, TradeDirection::AtoB), Ok(false));
        assert_eq!(compute_fee_token_is_a(0, TradeDirection::BtoA), Ok(true));
        // OnlyB (1) and Compounding (2): always token B, regardless of direction.
        assert_eq!(compute_fee_token_is_a(1, TradeDirection::AtoB), Ok(false));
        assert_eq!(compute_fee_token_is_a(1, TradeDirection::BtoA), Ok(false));
        assert_eq!(compute_fee_token_is_a(2, TradeDirection::AtoB), Ok(false));
        assert_eq!(compute_fee_token_is_a(2, TradeDirection::BtoA), Ok(false));
        // Unknown collect_fee_mode surfaces the raw value as an error.
        assert_eq!(compute_fee_token_is_a(7, TradeDirection::AtoB), Err(7));
    }

    /// The two on-chain enum decoders: valid discriminants map, out-of-range
    /// values surface the raw byte as an error.
    #[test]
    fn enum_from_u8_decoders() {
        assert_eq!(TradeDirection::from_u8(0), Ok(TradeDirection::AtoB));
        assert_eq!(TradeDirection::from_u8(1), Ok(TradeDirection::BtoA));
        assert_eq!(TradeDirection::from_u8(2), Err(2));

        assert_eq!(
            MeteoraDammV2LiquidityEventKind::from_u8(0),
            Ok(MeteoraDammV2LiquidityEventKind::Add)
        );
        assert_eq!(
            MeteoraDammV2LiquidityEventKind::from_u8(1),
            Ok(MeteoraDammV2LiquidityEventKind::Remove)
        );
        assert_eq!(MeteoraDammV2LiquidityEventKind::from_u8(9), Err(9));
    }
}
