use chrono::{TimeZone, Utc};
use serde_json::json;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use yog_core::domain::{
    MeteoraDammV2FundRewardEvent, MeteoraDammV2InitializeRewardEvent, MeteoraDammV2RewardChange,
    MeteoraDammV2UpdateRewardDurationEvent, MeteoraDammV2UpdateRewardFunderEvent,
    MeteoraDammV2WithdrawDeadLiquidityRewardEvent, MeteoraDammV2WithdrawIneligibleRewardEvent,
    RewardChangeEvent,
};

use super::RewardChangeEventResponse;

fn key(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn block(change: MeteoraDammV2RewardChange) -> serde_json::Value {
    let wire = serde_json::to_value(RewardChangeEventResponse::from(
        RewardChangeEvent::MeteoraDammV2(change),
    ))
    .unwrap();
    assert_eq!(wire["poolAddress"], key(1).to_string());
    assert_eq!(wire["eventIndex"], 2);
    wire["meteoraDammV2"].clone()
}

macro_rules! event {
    ($ty:ident { $($field:ident: $value:expr),* $(,)? }) => {
        $ty {
            pool_address: key(1),
            signature: Signature::from([2; 64]),
            timestamp: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
            slot: 9,
            transaction_index: None,
            event_index: 2,
            $($field: $value),*
        }
    };
}

/// Mutation: two variants or two fields swapped in the conversion, or the tag renamed — a
/// change read as another kind, a field as another, or its kind under another name.
#[test]
fn each_change_says_its_kind_and_its_own_fields() {
    let k = |seed: u8| key(seed).to_string();

    let initialize = block(MeteoraDammV2RewardChange::Initialize(event!(
        MeteoraDammV2InitializeRewardEvent {
            reward_mint: key(10),
            funder: key(11),
            creator: key(12),
            reward_index: 1,
            reward_duration: 3600,
        }
    )));
    let fund = block(MeteoraDammV2RewardChange::Fund(event!(
        MeteoraDammV2FundRewardEvent {
            funder: key(13),
            mint_reward: key(14),
            reward_index: 1,
            amount: 11,
            transfer_fee_excluded_amount_in: 12,
            reward_duration_end: 13,
            pre_reward_rate: 14,
            post_reward_rate: 1 << 64,
        }
    )));
    let duration = block(MeteoraDammV2RewardChange::DurationChange(event!(
        MeteoraDammV2UpdateRewardDurationEvent {
            reward_index: 1,
            old_reward_duration: 21,
            new_reward_duration: 22,
        }
    )));
    let funder = block(MeteoraDammV2RewardChange::FunderChange(event!(
        MeteoraDammV2UpdateRewardFunderEvent {
            reward_index: 1,
            old_funder: key(15),
            new_funder: key(16),
        }
    )));
    let ineligible = block(MeteoraDammV2RewardChange::IneligibleWithdrawal(event!(
        MeteoraDammV2WithdrawIneligibleRewardEvent {
            reward_mint: key(17),
            amount: 31,
        }
    )));
    let dead = block(MeteoraDammV2RewardChange::DeadLiquidityWithdrawal(event!(
        MeteoraDammV2WithdrawDeadLiquidityRewardEvent {
            reward_mint: key(18),
            amount: 41,
        }
    )));

    assert_eq!(
        initialize,
        json!({
            "kind": "initialize", "rewardMint": k(10), "funder": k(11), "creator": k(12),
            "rewardIndex": 1, "rewardDuration": "3600"
        })
    );
    assert_eq!(
        fund,
        json!({
            "kind": "fund", "funder": k(13), "rewardMint": k(14), "rewardIndex": 1,
            "amount": "11", "transferFeeExcludedAmountIn": "12", "rewardDurationEnd": "13",
            "preRewardRate": "14", "postRewardRate": "18446744073709551616"
        })
    );
    assert_eq!(
        duration,
        json!({
            "kind": "duration_change", "rewardIndex": 1, "oldRewardDuration": "21",
            "newRewardDuration": "22"
        })
    );
    assert_eq!(
        funder,
        json!({"kind": "funder_change", "rewardIndex": 1, "oldFunder": k(15), "newFunder": k(16)})
    );
    assert_eq!(
        ineligible,
        json!({"kind": "ineligible_withdrawal", "rewardMint": k(17), "amount": "31"})
    );
    assert_eq!(
        dead,
        json!({"kind": "dead_liquidity_withdrawal", "rewardMint": k(18), "amount": "41"})
    );
}
