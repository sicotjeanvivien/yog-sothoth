use super::*;
use async_trait::async_trait;
use chrono::DateTime;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use std::sync::Mutex;
use yog_core::RepositoryResult;
use yog_core::domain::{
    MeteoraDammV2LiquidityEventKind, MeteoraDammV2PoolPropertiesRepository,
    MeteoraDammV2SplitAmounts, MeteoraDammV2SplitNumerators, MeteoraDammV2SplitPositionState, Pool,
    PoolCurrentStateRepository, PoolCurrentStateUpsert, PoolRepository, TradeDirection,
};

type Calls = Arc<Mutex<Vec<&'static str>>>;

fn rec(calls: &Calls, what: &'static str) {
    calls.lock().unwrap().push(what);
}

// Write-only ring-2 repos: a single `insert` that records its label.
macro_rules! insert_only_mock {
    ($mock:ident, $repo:path, $event:ty, $label:literal) => {
        struct $mock(Calls);
        #[async_trait]
        impl $repo for $mock {
            async fn insert(&self, _e: &$event) -> RepositoryResult<()> {
                rec(&self.0, $label);
                Ok(())
            }
        }
    };
}
insert_only_mock!(
    MockCreate,
    MeteoraDammV2CreatePositionEventRepository,
    MeteoraDammV2CreatePositionEvent,
    "insert:create_position"
);
insert_only_mock!(
    MockClose,
    MeteoraDammV2ClosePositionEventRepository,
    MeteoraDammV2ClosePositionEvent,
    "insert:close_position"
);
insert_only_mock!(
    MockLock,
    MeteoraDammV2LockPositionEventRepository,
    MeteoraDammV2LockPositionEvent,
    "insert:lock_position"
);
insert_only_mock!(
    MockPermLock,
    MeteoraDammV2PermanentLockPositionEventRepository,
    MeteoraDammV2PermanentLockPositionEvent,
    "insert:permanent_lock_position"
);
insert_only_mock!(
    MockInit,
    MeteoraDammV2InitializePoolEventRepository,
    MeteoraDammV2InitializePoolEvent,
    "insert:initialize_pool"
);
insert_only_mock!(
    MockSetStatus,
    MeteoraDammV2SetPoolStatusEventRepository,
    MeteoraDammV2SetPoolStatusEvent,
    "insert:set_pool_status"
);
insert_only_mock!(
    MockUpdateFees,
    MeteoraDammV2UpdatePoolFeesEventRepository,
    MeteoraDammV2UpdatePoolFeesEvent,
    "insert:update_pool_fees"
);
insert_only_mock!(
    MockClaimProtocolFee,
    MeteoraDammV2ClaimProtocolFeeEventRepository,
    MeteoraDammV2ClaimProtocolFeeEvent,
    "insert:claim_protocol_fee"
);
insert_only_mock!(
    MockInitializeReward,
    MeteoraDammV2InitializeRewardEventRepository,
    MeteoraDammV2InitializeRewardEvent,
    "insert:initialize_reward"
);
insert_only_mock!(
    MockFundReward,
    MeteoraDammV2FundRewardEventRepository,
    MeteoraDammV2FundRewardEvent,
    "insert:fund_reward"
);
insert_only_mock!(
    MockWithdrawIneligibleReward,
    MeteoraDammV2WithdrawIneligibleRewardEventRepository,
    MeteoraDammV2WithdrawIneligibleRewardEvent,
    "insert:withdraw_ineligible_reward"
);
insert_only_mock!(
    MockUpdateRewardDuration,
    MeteoraDammV2UpdateRewardDurationEventRepository,
    MeteoraDammV2UpdateRewardDurationEvent,
    "insert:update_reward_duration"
);
insert_only_mock!(
    MockUpdateRewardFunder,
    MeteoraDammV2UpdateRewardFunderEventRepository,
    MeteoraDammV2UpdateRewardFunderEvent,
    "insert:update_reward_funder"
);
insert_only_mock!(
    MockWithdrawDeadLiquidityReward,
    MeteoraDammV2WithdrawDeadLiquidityRewardEventRepository,
    MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
    "insert:withdraw_dead_liquidity_reward"
);
insert_only_mock!(
    MockSplitPosition,
    MeteoraDammV2SplitPositionEventRepository,
    MeteoraDammV2SplitPositionEvent,
    "insert:split_position"
);

// Ring-1 repos: record `insert`; their read methods are never hit by
// `persist()`, so they stub out.
struct MockSwap(Calls);
#[async_trait]
impl MeteoraDammV2SwapEventRepository for MockSwap {
    async fn insert(&self, _e: &MeteoraDammV2SwapEvent) -> RepositoryResult<()> {
        rec(&self.0, "insert:swap");
        Ok(())
    }
}
struct MockLiquidity(Calls);
#[async_trait]
impl MeteoraDammV2LiquidityEventRepository for MockLiquidity {
    async fn insert(&self, _e: &MeteoraDammV2LiquidityEvent) -> RepositoryResult<()> {
        rec(&self.0, "insert:liquidity");
        Ok(())
    }
}
struct MockClaimFee(Calls);
#[async_trait]
impl MeteoraDammV2ClaimPositionFeeEventRepository for MockClaimFee {
    async fn insert(&self, _e: &MeteoraDammV2ClaimPositionFeeEvent) -> RepositoryResult<()> {
        rec(&self.0, "insert:claim_position_fee");
        Ok(())
    }
}
struct MockClaimReward(Calls);
#[async_trait]
impl MeteoraDammV2ClaimRewardEventRepository for MockClaimReward {
    async fn insert(&self, _e: &MeteoraDammV2ClaimRewardEvent) -> RepositoryResult<()> {
        rec(&self.0, "insert:claim_reward");
        Ok(())
    }
}

// PoolMaintenance's two underlying repos. `pool:upsert` ⇒ upsert_pool_full,
// `pool:touch` ⇒ touch_pool, `pcs:upsert` ⇒ current-state projection.
struct MockPoolRepo(Calls);
#[async_trait]
impl PoolRepository for MockPoolRepo {
    async fn upsert(&self, _p: &Pool) -> RepositoryResult<()> {
        rec(&self.0, "pool:upsert");
        Ok(())
    }
    async fn touch_last_seen(&self, _: &Pubkey) -> RepositoryResult<()> {
        rec(&self.0, "pool:touch");
        Ok(())
    }
    async fn set_fee_bps(&self, _: &Pubkey, _: rust_decimal::Decimal) -> RepositoryResult<()> {
        rec(&self.0, "pool:set_fee_bps");
        Ok(())
    }
}

// The DAMM v2 pool-properties satellite (migration 036). `pool:set_fee_config`
// keeps its historical call label so the existing assertions still read the same
// sequence — only the repository it lands on changed.
struct MockPoolProperties(Calls);
#[async_trait]
impl MeteoraDammV2PoolPropertiesRepository for MockPoolProperties {
    async fn set_fee_config(&self, _: &Pubkey, _: &str, _: bool) -> RepositoryResult<()> {
        rec(&self.0, "pool:set_fee_config");
        Ok(())
    }
    async fn set_has_dynamic_fee(&self, _: &Pubkey, _: bool) -> RepositoryResult<()> {
        rec(&self.0, "pool:set_has_dynamic_fee");
        Ok(())
    }
    async fn find_by_pool(
        &self,
        _: &Pubkey,
    ) -> RepositoryResult<Option<yog_core::domain::MeteoraDammV2PoolProperties>> {
        unreachable!("read side is the api's, never exercised by the persistor")
    }
}

struct MockPcsRepo(Calls);
#[async_trait]
impl PoolCurrentStateRepository for MockPcsRepo {
    async fn upsert(&self, _: &PoolCurrentStateUpsert) -> RepositoryResult<bool> {
        rec(&self.0, "pcs:upsert");
        Ok(true)
    }
}

fn pk(b: u8) -> Pubkey {
    Pubkey::new_from_array([b; 32])
}
fn ts() -> DateTime<chrono::Utc> {
    DateTime::from_timestamp(1_700_000_000, 0).unwrap()
}
fn sg() -> Signature {
    Signature::from([0u8; 64])
}

fn build(calls: Calls) -> MeteoraDammV2EventPersistor {
    let repos = DammV2Repos {
        swap_event: Arc::new(MockSwap(calls.clone())),
        liquidity_event: Arc::new(MockLiquidity(calls.clone())),
        claim_position_fee: Arc::new(MockClaimFee(calls.clone())),
        claim_protocol_fee: Arc::new(MockClaimProtocolFee(calls.clone())),
        claim_reward: Arc::new(MockClaimReward(calls.clone())),
        initialize_reward: Arc::new(MockInitializeReward(calls.clone())),
        fund_reward: Arc::new(MockFundReward(calls.clone())),
        withdraw_ineligible_reward: Arc::new(MockWithdrawIneligibleReward(calls.clone())),
        update_reward_duration: Arc::new(MockUpdateRewardDuration(calls.clone())),
        update_reward_funder: Arc::new(MockUpdateRewardFunder(calls.clone())),
        withdraw_dead_liquidity_reward: Arc::new(MockWithdrawDeadLiquidityReward(calls.clone())),
        create_position: Arc::new(MockCreate(calls.clone())),
        close_position: Arc::new(MockClose(calls.clone())),
        lock_position: Arc::new(MockLock(calls.clone())),
        permanent_lock_position: Arc::new(MockPermLock(calls.clone())),
        initialize_pool: Arc::new(MockInit(calls.clone())),
        set_pool_status: Arc::new(MockSetStatus(calls.clone())),
        split_position: Arc::new(MockSplitPosition(calls.clone())),
        update_pool_fees: Arc::new(MockUpdateFees(calls.clone())),
        pool_properties: Arc::new(MockPoolProperties(calls.clone())),
    };
    let pm = Arc::new(PoolMaintenance::new(
        Arc::new(MockPoolRepo(calls.clone())),
        Arc::new(MockPcsRepo(calls.clone())),
    ));
    MeteoraDammV2EventPersistor::new(repos, pm)
}

async fn route(
    p: &MeteoraDammV2EventPersistor,
    calls: &Calls,
    ev: MeteoraDammV2Event,
) -> Vec<&'static str> {
    calls.lock().unwrap().clear();
    p.persist(&ev).await;
    calls.lock().unwrap().clone()
}

fn split_amounts() -> MeteoraDammV2SplitAmounts {
    MeteoraDammV2SplitAmounts {
        permanent_locked_liquidity: 0,
        unlocked_liquidity: 10,
        vested_liquidity: 0,
        fee_a: 1,
        fee_b: 2,
        reward_0: 0,
        reward_1: 0,
    }
}
fn split_state() -> MeteoraDammV2SplitPositionState {
    MeteoraDammV2SplitPositionState {
        unlocked_liquidity: 10,
        permanent_locked_liquidity: 0,
        vested_liquidity: 0,
        fee_a: 1,
        fee_b: 2,
        reward_0: 0,
        reward_1: 0,
    }
}
fn split_numerators() -> MeteoraDammV2SplitNumerators {
    MeteoraDammV2SplitNumerators {
        unlocked_liquidity: 500_000_000,
        permanent_locked_liquidity: 0,
        fee_a: 0,
        fee_b: 0,
        reward_0: 0,
        reward_1: 0,
        inner_vesting_liquidity: 0,
    }
}

fn swap() -> MeteoraDammV2SwapEvent {
    MeteoraDammV2SwapEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        trade_direction: TradeDirection::AtoB,
        amount_a: 1,
        amount_b: 2,
        reserve_a_after: 10,
        reserve_b_after: 20,
        next_sqrt_price: 123,
        claiming_fee: 0,
        protocol_fee: 0,
        compounding_fee: 0,
        referral_fee: 0,
        fee_token_is_a: true,
    }
}
fn liquidity() -> MeteoraDammV2LiquidityEvent {
    MeteoraDammV2LiquidityEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        liquidity_event_kind: MeteoraDammV2LiquidityEventKind::Add,
        amount_a: 1,
        amount_b: 2,
        liquidity_delta: 5,
        reserve_a_after: 10,
        reserve_b_after: 20,
        position: pk(4),
        owner: pk(5),
    }
}

#[tokio::test]
async fn persist_routes_each_event_to_its_repo_and_recipe() {
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let p = build(calls.clone());

    // swap / liquidity: full upsert, insert, then current-state projection.
    assert_eq!(
        route(&p, &calls, MeteoraDammV2Event::Swap(swap())).await,
        ["pool:upsert", "insert:swap", "pcs:upsert"]
    );
    assert_eq!(
        route(&p, &calls, MeteoraDammV2Event::Liquidity(liquidity())).await,
        ["pool:upsert", "insert:liquidity", "pcs:upsert"]
    );

    // claim_position_fee / claim_reward: touch + insert.
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::ClaimPositionFee(MeteoraDammV2ClaimPositionFeeEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                position: pk(4),
                owner: pk(5),
                fee_a_claimed: 1,
                fee_b_claimed: 2,
            })
        )
        .await,
        ["pool:touch", "insert:claim_position_fee"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::ClaimReward(MeteoraDammV2ClaimRewardEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                position: pk(4),
                owner: pk(5),
                mint_reward: pk(6),
                reward_index: 0,
                total_reward: 9,
            })
        )
        .await,
        ["pool:touch", "insert:claim_reward"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::ClaimProtocolFee(MeteoraDammV2ClaimProtocolFeeEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                token_a_amount: 0,
                token_b_amount: 1_421_627_556,
            })
        )
        .await,
        ["pool:touch", "insert:claim_protocol_fee"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::InitializeReward(MeteoraDammV2InitializeRewardEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                reward_mint: pk(6),
                funder: pk(7),
                creator: pk(7),
                reward_index: 0,
                reward_duration: 604_800,
            })
        )
        .await,
        ["pool:touch", "insert:initialize_reward"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::FundReward(MeteoraDammV2FundRewardEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                funder: pk(7),
                mint_reward: pk(6),
                reward_index: 0,
                amount: 100_000_000_000,
                transfer_fee_excluded_amount_in: 100_000_000_000,
                reward_duration_end: 1_785_727_188,
                pre_reward_rate: 0,
                post_reward_rate: 3_050_056_890_494_304_169_312_169,
            })
        )
        .await,
        ["pool:touch", "insert:fund_reward"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::WithdrawIneligibleReward(
                MeteoraDammV2WithdrawIneligibleRewardEvent {
                    pool_address: pk(1),
                    signature: sg(),
                    timestamp: ts(),
                    reward_mint: pk(6),
                    amount: 0,
                }
            )
        )
        .await,
        ["pool:touch", "insert:withdraw_ineligible_reward"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::UpdateRewardDuration(MeteoraDammV2UpdateRewardDurationEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                reward_index: 1,
                old_reward_duration: 604_800,
                new_reward_duration: 1_209_600,
            })
        )
        .await,
        ["pool:touch", "insert:update_reward_duration"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::UpdateRewardFunder(MeteoraDammV2UpdateRewardFunderEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                reward_index: 0,
                old_funder: pk(7),
                new_funder: pk(8),
            })
        )
        .await,
        ["pool:touch", "insert:update_reward_funder"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::WithdrawDeadLiquidityReward(
                MeteoraDammV2WithdrawDeadLiquidityRewardEvent {
                    pool_address: pk(1),
                    signature: sg(),
                    timestamp: ts(),
                    reward_mint: pk(6),
                    // cp-amm only emits this event when the amount is > 0.
                    amount: 42_000,
                }
            )
        )
        .await,
        ["pool:touch", "insert:withdraw_dead_liquidity_reward"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::SplitPosition(MeteoraDammV2SplitPositionEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                first_owner: pk(2),
                second_owner: pk(3),
                first_position: pk(4),
                second_position: pk(5),
                current_sqrt_price: 1,
                amounts: split_amounts(),
                first_position_after: split_state(),
                second_position_after: split_state(),
                numerators: split_numerators(),
            })
        )
        .await,
        ["pool:touch", "insert:split_position"]
    );

    // create / close: touch + insert.
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::CreatePosition(MeteoraDammV2CreatePositionEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                owner: pk(5),
                position: pk(4),
                position_nft_mint: pk(7),
            })
        )
        .await,
        ["pool:touch", "insert:create_position"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::ClosePosition(MeteoraDammV2ClosePositionEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                owner: pk(5),
                position: pk(4),
                position_nft_mint: pk(7),
            })
        )
        .await,
        ["pool:touch", "insert:close_position"]
    );

    // lock / permanent-lock: touch + insert.
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::LockPosition(MeteoraDammV2LockPositionEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                position: pk(4),
                owner: pk(5),
                vesting: pk(8),
                cliff_point: 1,
                period_frequency: 1,
                cliff_unlock_liquidity: 1,
                liquidity_per_period: 0,
                number_of_period: 0,
            })
        )
        .await,
        ["pool:touch", "insert:lock_position"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::PermanentLockPosition(MeteoraDammV2PermanentLockPositionEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                position: pk(4),
                lock_liquidity_amount: 1,
                total_permanent_locked_liquidity: 1,
            })
        )
        .await,
        ["pool:touch", "insert:permanent_lock_position"]
    );

    // initialize_pool: full upsert + decode/record fee + insert, NO
    // projection. The 27-byte fee blob (numerator 2_500_000, mode 0)
    // decodes cleanly, so the fee_bps step fires between upsert and insert.
    // 31 bytes: enough for both decodes — set_fee_bps (cliff numerator @ 0..8,
    // 2_500_000 → 25 bps) and set_fee_config (mode @26, dynamic-fee tag @30).
    // All-zero tail → mode 0, no periods, no dynamic fee → constant fee shape.
    let mut fee_blob = vec![0u8; 31];
    fee_blob[0..8].copy_from_slice(&2_500_000u64.to_le_bytes());
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::InitializePool(MeteoraDammV2InitializePoolEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                token_a_mint: pk(2),
                token_b_mint: pk(3),
                creator: pk(9),
                payer: pk(10),
                alpha_vault: pk(11),
                sqrt_min_price: 1,
                sqrt_max_price: 100,
                sqrt_price: 50,
                liquidity: 1000,
                activation_type: 0,
                activation_point: 0,
                collect_fee_mode: 0,
                pool_type: 0,
                token_a_flag: 0,
                token_b_flag: 0,
                token_a_amount: 1,
                token_b_amount: 2,
                total_amount_a: 1,
                total_amount_b: 2,
                pool_fees_raw: fee_blob,
            })
        )
        .await,
        [
            "pool:upsert",
            "pool:set_fee_bps",
            "pool:set_fee_config",
            "insert:initialize_pool"
        ]
    );

    // set_pool_status / update_pool_fees: touch + insert.
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::SetPoolStatus(MeteoraDammV2SetPoolStatusEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                status: 1,
            })
        )
        .await,
        ["pool:touch", "insert:set_pool_status"]
    );
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::UpdatePoolFees(MeteoraDammV2UpdatePoolFeesEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                operator: pk(12),
                // cliff_fee_numerator = Some(2_500_000) → 25 bps: refreshes
                // the fee tier, so set_fee_bps fires between touch and insert.
                // The blob ends there — an older build with no dynamic field —
                // so the dynamic fee is left alone.
                params_raw: vec![1, 160, 37, 38, 0, 0, 0, 0, 0],
            })
        )
        .await,
        ["pool:touch", "pool:set_fee_bps", "insert:update_pool_fees"]
    );
}

/// An operator can toggle the dynamic fee in the same event, and
/// `has_dynamic_fee` must follow — it is written at genesis only otherwise, and
/// the pool detail sheet shows it.
#[tokio::test]
async fn update_pool_fees_enabling_the_dynamic_fee_writes_the_flag() {
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let p = build(calls.clone());

    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::UpdatePoolFees(MeteoraDammV2UpdatePoolFeesEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                operator: pk(12),
                params_raw: update_pool_fees_blob(Some(true)),
            })
        )
        .await,
        [
            "pool:touch",
            "pool:set_fee_bps",
            "pool:set_has_dynamic_fee",
            "insert:update_pool_fees"
        ]
    );
}

/// The disable case, which no fixture shows: cp-amm signals it with
/// `Some(DynamicFeeParameters::default())` — an all-zero payload. It must reach
/// the repository just like enabling does, not be mistaken for "no change".
#[tokio::test]
async fn update_pool_fees_disabling_the_dynamic_fee_also_writes_the_flag() {
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let p = build(calls.clone());

    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::UpdatePoolFees(MeteoraDammV2UpdatePoolFeesEvent {
                pool_address: pk(1),
                signature: sg(),
                timestamp: ts(),
                operator: pk(12),
                params_raw: update_pool_fees_blob(Some(false)),
            })
        )
        .await,
        [
            "pool:touch",
            "pool:set_fee_bps",
            "pool:set_has_dynamic_fee",
            "insert:update_pool_fees"
        ]
    );
}

/// `cliff_fee_numerator = Some(25 bps)` followed by the dynamic field:
/// `None` → absent, `Some(true)` → real values, `Some(false)` → all zeros.
fn update_pool_fees_blob(dynamic: Option<bool>) -> Vec<u8> {
    let mut blob = vec![1u8, 160, 37, 38, 0, 0, 0, 0, 0];
    match dynamic {
        None => blob.push(0),
        Some(enabled) => {
            blob.push(1);
            if enabled {
                blob.extend_from_slice(&1u16.to_le_bytes());
                blob.extend_from_slice(&1_844_674_407_370_955u128.to_le_bytes());
                blob.extend_from_slice(&10u16.to_le_bytes());
                blob.extend_from_slice(&120u16.to_le_bytes());
                blob.extend_from_slice(&5000u16.to_le_bytes());
                blob.extend_from_slice(&14_460_000u32.to_le_bytes());
                blob.extend_from_slice(&1224u32.to_le_bytes());
            } else {
                blob.extend_from_slice(&[0u8; 32]);
            }
        }
    }
    blob
}
