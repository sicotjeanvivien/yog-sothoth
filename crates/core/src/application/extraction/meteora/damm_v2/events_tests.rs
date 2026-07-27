use super::*;
// `try_from_slice` comes from this trait; the parent module no longer imports
// borsh now that each event struct lives in its own file.
use borsh::BorshDeserialize;

/// Sanity check: discriminators are 8 bytes and stable across runs.
#[test]
fn discriminators_are_eight_bytes() {
    assert_eq!(discriminator_swap2().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_liquidity_change().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_claim_position_fee().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_claim_reward().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_claim_protocol_fee().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_initialize_reward().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_fund_reward().len(), DISCRIMINATOR_LEN);
    assert_eq!(
        discriminator_withdraw_ineligible_reward().len(),
        DISCRIMINATOR_LEN
    );
    assert_eq!(
        discriminator_update_reward_duration().len(),
        DISCRIMINATOR_LEN
    );
    assert_eq!(
        discriminator_update_reward_funder().len(),
        DISCRIMINATOR_LEN
    );
    assert_eq!(
        discriminator_withdraw_dead_liquidity_reward().len(),
        DISCRIMINATOR_LEN
    );
    assert_eq!(discriminator_create_position().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_close_position().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_lock_position().len(), DISCRIMINATOR_LEN);
    assert_eq!(
        discriminator_permanent_lock_position().len(),
        DISCRIMINATOR_LEN
    );
    assert_eq!(discriminator_initialize_pool().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_set_pool_status().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_update_pool_fees().len(), DISCRIMINATOR_LEN);
}

/// Sanity check: each event has a distinct discriminator. If two events
/// ever collide (extremely unlikely with sha256), our dispatch logic
/// would silently mis-decode one as the other.
#[test]
fn discriminators_are_unique() {
    let all = [
        discriminator_swap2(),
        discriminator_liquidity_change(),
        discriminator_claim_position_fee(),
        discriminator_claim_reward(),
        discriminator_claim_protocol_fee(),
        discriminator_initialize_reward(),
        discriminator_fund_reward(),
        discriminator_withdraw_ineligible_reward(),
        discriminator_update_reward_duration(),
        discriminator_update_reward_funder(),
        discriminator_withdraw_dead_liquidity_reward(),
        discriminator_create_position(),
        discriminator_close_position(),
        discriminator_lock_position(),
        discriminator_permanent_lock_position(),
        discriminator_initialize_pool(),
        discriminator_set_pool_status(),
        discriminator_update_pool_fees(),
    ];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(all[i], all[j], "discriminator collision at {i}/{j}");
        }
    }
}

// ---------------------------------------------------------------------------
// Layout pinning — events mirrored without an on-chain fixture
// ---------------------------------------------------------------------------
//
// EvtUpdateRewardDuration / EvtUpdateRewardFunder / EvtWithdrawDeadLiquidityReward
// have no captured mainnet transaction, so nothing proves our mirror against
// bytes the program actually emitted. These tests are the next best thing: they
// build the payload byte by byte from the cp-amm source layout and assert both
// the total size and that every field lands at its expected offset.
//
// What they catch: a future edit that reorders, resizes or inserts a field in
// our mirror. What they CANNOT catch: a misreading of the cp-amm source in the
// first place — the bytes here are derived from the same reading. Only a real
// fixture closes that gap. Replace these with a fixture test when a transaction
// is captured.

/// Little-endian byte helper mirroring borsh's integer encoding.
fn payload_update_reward_duration() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&[1u8; 32]); // pool
    v.push(1); // reward_index
    v.extend_from_slice(&604_800u64.to_le_bytes()); // old_reward_duration
    v.extend_from_slice(&1_209_600u64.to_le_bytes()); // new_reward_duration
    v
}

#[test]
fn update_reward_duration_layout_is_pinned() {
    let bytes = payload_update_reward_duration();
    // 32 (pool) + 1 (u8) + 8 + 8. Borsh does not pad, so the u8 sits flush
    // between the pubkey and the first u64.
    assert_eq!(bytes.len(), 49, "payload size drift");

    let e = EvtUpdateRewardDuration::try_from_slice(&bytes).expect("deserialize");
    assert_eq!(e.pool, Pubkey::new_from_array([1u8; 32]));
    assert_eq!(e.reward_index, 1);
    assert_eq!(e.old_reward_duration, 604_800);
    assert_eq!(e.new_reward_duration, 1_209_600);
}

#[test]
fn update_reward_funder_layout_is_pinned() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[1u8; 32]); // pool
    bytes.push(0); // reward_index
    bytes.extend_from_slice(&[2u8; 32]); // old_funder
    bytes.extend_from_slice(&[3u8; 32]); // new_funder
    assert_eq!(bytes.len(), 97, "payload size drift"); // 32 + 1 + 32 + 32

    let e = EvtUpdateRewardFunder::try_from_slice(&bytes).expect("deserialize");
    assert_eq!(e.pool, Pubkey::new_from_array([1u8; 32]));
    assert_eq!(e.reward_index, 0);
    assert_eq!(e.old_funder, Pubkey::new_from_array([2u8; 32]));
    assert_eq!(e.new_funder, Pubkey::new_from_array([3u8; 32]));
}

#[test]
fn withdraw_dead_liquidity_reward_layout_is_pinned() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[1u8; 32]); // pool
    bytes.extend_from_slice(&[2u8; 32]); // reward_mint
    bytes.extend_from_slice(&42_000u64.to_le_bytes()); // amount
    assert_eq!(bytes.len(), 72, "payload size drift"); // 32 + 32 + 8

    let e = EvtWithdrawDeadLiquidityReward::try_from_slice(&bytes).expect("deserialize");
    assert_eq!(e.pool, Pubkey::new_from_array([1u8; 32]));
    assert_eq!(e.reward_mint, Pubkey::new_from_array([2u8; 32]));
    assert_eq!(e.amount, 42_000);
}

/// EvtWithdrawDeadLiquidityReward and EvtWithdrawIneligibleReward are
/// byte-identical in shape (72 B, pool + reward_mint + amount). Only the
/// discriminator tells them apart — so the same payload must decode to both,
/// and dispatch must never rely on size.
#[test]
fn dead_liquidity_and_ineligible_reward_share_a_shape() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[1u8; 32]);
    bytes.extend_from_slice(&[2u8; 32]);
    bytes.extend_from_slice(&7u64.to_le_bytes());

    let dead = EvtWithdrawDeadLiquidityReward::try_from_slice(&bytes).expect("dead");
    let inel = EvtWithdrawIneligibleReward::try_from_slice(&bytes).expect("ineligible");
    assert_eq!(dead.pool, inel.pool);
    assert_eq!(dead.reward_mint, inel.reward_mint);
    assert_eq!(dead.amount, inel.amount);
    assert_ne!(
        discriminator_withdraw_dead_liquidity_reward(),
        discriminator_withdraw_ineligible_reward(),
        "identical shapes must be separated by their discriminator"
    );
}
