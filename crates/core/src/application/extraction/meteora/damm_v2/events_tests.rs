use super::*;

/// Sanity check: discriminators are 8 bytes and stable across runs.
#[test]
fn discriminators_are_eight_bytes() {
    assert_eq!(discriminator_swap2().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_liquidity_change().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_claim_position_fee().len(), DISCRIMINATOR_LEN);
    assert_eq!(discriminator_claim_reward().len(), DISCRIMINATOR_LEN);
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
