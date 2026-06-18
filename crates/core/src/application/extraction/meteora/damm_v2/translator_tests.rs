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
