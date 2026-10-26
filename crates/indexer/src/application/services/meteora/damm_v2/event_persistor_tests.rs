use super::*;
use async_trait::async_trait;
use chrono::DateTime;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use std::sync::Mutex;
use yog_core::domain::{
    InsertOutcome, MeteoraDammV2LiquidityEventKind, MeteoraDammV2SplitAmounts,
    MeteoraDammV2SplitNumerators, MeteoraDammV2SplitPositionState, Pool,
    PoolCurrentStateRepository, PoolCurrentStateUpsert, PoolCurrentStateUpsertOutcome,
    PoolRepository, TradeDirection,
};
use yog_core::{RepositoryError, RepositoryResult};

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
            async fn insert(&self, _e: &$event) -> RepositoryResult<InsertOutcome> {
                rec(&self.0, $label);
                Ok(InsertOutcome::Inserted)
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
struct MockInit(Calls, InsertOutcome);
#[async_trait]
impl MeteoraDammV2InitializePoolEventRepository for MockInit {
    async fn insert(
        &self,
        _e: &MeteoraDammV2InitializePoolEvent,
    ) -> RepositoryResult<InsertOutcome> {
        rec(&self.0, "insert:initialize_pool");
        Ok(self.1)
    }
}
insert_only_mock!(
    MockSetStatus,
    MeteoraDammV2SetPoolStatusEventRepository,
    MeteoraDammV2SetPoolStatusEvent,
    "insert:set_pool_status"
);
struct MockUpdateFees(Calls, InsertOutcome);
#[async_trait]
impl MeteoraDammV2UpdatePoolFeesEventRepository for MockUpdateFees {
    async fn insert(
        &self,
        _e: &MeteoraDammV2UpdatePoolFeesEvent,
    ) -> RepositoryResult<InsertOutcome> {
        rec(&self.0, "insert:update_pool_fees");
        Ok(self.1)
    }
}
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

// Ring-1 repos: record `insert`; `persist()` never reads, so the rest is stubbed.
/// Carries the outcome it should report, so a test can drive `persist()`'s `Skipped` branch.
struct MockSwap(Calls, InsertOutcome);
#[async_trait]
impl MeteoraDammV2SwapEventRepository for MockSwap {
    async fn insert(&self, _e: &MeteoraDammV2SwapEvent) -> RepositoryResult<InsertOutcome> {
        rec(&self.0, "insert:swap");
        Ok(self.1)
    }
}
struct MockLiquidity(Calls);
#[async_trait]
impl MeteoraDammV2LiquidityEventRepository for MockLiquidity {
    async fn insert(&self, _e: &MeteoraDammV2LiquidityEvent) -> RepositoryResult<InsertOutcome> {
        rec(&self.0, "insert:liquidity");
        Ok(InsertOutcome::Inserted)
    }
}
struct MockClaimFee(Calls);
#[async_trait]
impl MeteoraDammV2ClaimPositionFeeEventRepository for MockClaimFee {
    async fn insert(
        &self,
        _e: &MeteoraDammV2ClaimPositionFeeEvent,
    ) -> RepositoryResult<InsertOutcome> {
        rec(&self.0, "insert:claim_position_fee");
        Ok(InsertOutcome::Inserted)
    }
}
struct MockClaimReward(Calls);
#[async_trait]
impl MeteoraDammV2ClaimRewardEventRepository for MockClaimReward {
    async fn insert(&self, _e: &MeteoraDammV2ClaimRewardEvent) -> RepositoryResult<InsertOutcome> {
        rec(&self.0, "insert:claim_reward");
        Ok(InsertOutcome::Inserted)
    }
}

// PoolMaintenance's two repos: `pool:touch` is touch_pool, `pcs:upsert` the current-state projection.
// The flag makes `upsert` fail.
struct MockPoolRepo(Calls, bool);
#[async_trait]
impl PoolRepository for MockPoolRepo {
    async fn upsert(&self, _p: &Pool) -> RepositoryResult<()> {
        rec(&self.0, "pool:upsert");
        if self.1 {
            return Err(RepositoryError::Backend("deadlock detected".into()));
        }
        Ok(())
    }
    async fn touch_last_seen(&self, _: &Pubkey) -> RepositoryResult<()> {
        rec(&self.0, "pool:touch");
        Ok(())
    }
    async fn mark_needs_refresh(&self, _: &Pubkey) -> RepositoryResult<()> {
        rec(&self.0, "pool:mark_needs_refresh");
        Ok(())
    }
    /// Never called: the registry's account-derived columns are yog-context's alone.
    async fn set_registry_properties(
        &self,
        _: &Pubkey,
        _: &yog_core::domain::PoolRegistryProperties,
    ) -> RepositoryResult<()> {
        unreachable!("set_registry_properties belongs to yog-context, never the indexer")
    }
}

/// Carries the outcome it should report, so a test can drive the `same_slot_ambiguity` branch.
struct MockPcsRepo(Calls, PoolCurrentStateUpsertOutcome);
#[async_trait]
impl PoolCurrentStateRepository for MockPcsRepo {
    async fn upsert(
        &self,
        _: &PoolCurrentStateUpsert,
    ) -> RepositoryResult<PoolCurrentStateUpsertOutcome> {
        rec(&self.0, "pcs:upsert");
        Ok(self.1)
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
    build_with(calls, InsertOutcome::Inserted, applied_without_ambiguity())
}

fn build_with_swap_outcome(
    calls: Calls,
    swap_outcome: InsertOutcome,
) -> MeteoraDammV2EventPersistor {
    build_with(calls, swap_outcome, applied_without_ambiguity())
}

fn applied_without_ambiguity() -> PoolCurrentStateUpsertOutcome {
    PoolCurrentStateUpsertOutcome {
        applied: true,
        same_slot_ambiguity: false,
    }
}

fn build_with(
    calls: Calls,
    swap_outcome: InsertOutcome,
    pcs_outcome: PoolCurrentStateUpsertOutcome,
) -> MeteoraDammV2EventPersistor {
    build_all(
        calls,
        swap_outcome,
        pcs_outcome,
        false,
        InsertOutcome::Inserted,
    )
}

fn build_with_failing_pool_upsert(calls: Calls) -> MeteoraDammV2EventPersistor {
    build_all(
        calls,
        InsertOutcome::Inserted,
        applied_without_ambiguity(),
        true,
        InsertOutcome::Inserted,
    )
}

fn build_with_fee_insert_outcome(
    calls: Calls,
    outcome: InsertOutcome,
) -> MeteoraDammV2EventPersistor {
    build_all(
        calls,
        InsertOutcome::Inserted,
        applied_without_ambiguity(),
        false,
        outcome,
    )
}

fn build_all(
    calls: Calls,
    swap_outcome: InsertOutcome,
    pcs_outcome: PoolCurrentStateUpsertOutcome,
    pool_upsert_fails: bool,
    fee_insert_outcome: InsertOutcome,
) -> MeteoraDammV2EventPersistor {
    let repos = DammV2Repos {
        swap_event: Arc::new(MockSwap(calls.clone(), swap_outcome)),
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
        initialize_pool: Arc::new(MockInit(calls.clone(), fee_insert_outcome)),
        set_pool_status: Arc::new(MockSetStatus(calls.clone())),
        split_position: Arc::new(MockSplitPosition(calls.clone())),
        update_pool_fees: Arc::new(MockUpdateFees(calls.clone(), fee_insert_outcome)),
    };
    let pm = Arc::new(PoolMaintenance::new(
        Arc::new(MockPoolRepo(calls.clone(), pool_upsert_fails)),
        Arc::new(MockPcsRepo(calls.clone(), pcs_outcome)),
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
        slot: 1,
        transaction_index: None,
        event_index: 0,
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
        slot: 1,
        transaction_index: None,
        event_index: 0,
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

fn initialize_pool(pool_fees_raw: Vec<u8>) -> MeteoraDammV2InitializePoolEvent {
    MeteoraDammV2InitializePoolEvent {
        pool_address: pk(1),
        signature: sg(),
        timestamp: ts(),
        slot: 1,
        transaction_index: None,
        event_index: 0,
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
        pool_fees_raw,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                    slot: 1,
                    transaction_index: None,
                    event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                    slot: 1,
                    transaction_index: None,
                    event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
                position: pk(4),
                lock_liquidity_amount: 1,
                total_permanent_locked_liquidity: 1,
            })
        )
        .await,
        ["pool:touch", "insert:permanent_lock_position"]
    );

    // initialize_pool: upsert + insert, no projection; the blob is stored verbatim.
    // All-zero tail → mode 0, no periods, no dynamic fee → constant fee shape.
    let mut fee_blob = vec![0u8; 31];
    fee_blob[0..8].copy_from_slice(&2_500_000u64.to_le_bytes());
    assert_eq!(
        route(
            &p,
            &calls,
            MeteoraDammV2Event::InitializePool(initialize_pool(fee_blob))
        )
        .await,
        ["pool:upsert", "insert:initialize_pool"]
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
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
                slot: 1,
                transaction_index: None,
                event_index: 0,
                operator: pk(12),
                // A fee change flags the pool: yog-context re-reads the account.
                params_raw: vec![1, 160, 37, 38, 0, 0, 0, 0, 0],
            })
        )
        .await,
        [
            "pool:touch",
            "pool:mark_needs_refresh",
            "insert:update_pool_fees"
        ]
    );
}

// ── a skipped insert is counted, not merely tolerated ──────────────────────

/// Both counters, their relationship being the contract: rows written are `indexed − skipped`.
/// Mutation this is written against: deleting `persist()`'s `Skipped` branch, every other mock
/// reporting `Inserted`. Not `#[tokio::test]`: `with_local_recorder` binds the current thread.
#[test]
fn a_skipped_insert_is_counted_and_still_counts_as_processed() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(async {
                let calls: Calls = Arc::new(Mutex::new(Vec::new()));
                let p = build_with_swap_outcome(calls.clone(), InsertOutcome::Skipped);
                p.persist(&MeteoraDammV2Event::Swap(swap())).await;

                // The recipe still ran in full: a skip is not a failure.
                assert_eq!(
                    calls.lock().unwrap().clone(),
                    ["pool:upsert", "insert:swap", "pcs:upsert"]
                );
            });
    });

    // ⚠️ One snapshot, queried twice: `Snapshotter::snapshot` swaps counters to 0.
    let snapshot = snapshotter.snapshot().into_vec();
    let counter = |name: &str| {
        snapshot
            .iter()
            .find(|(key, _, _, _)| key.key().name() == name)
            .map(|(_, _, _, value)| value)
    };

    assert_eq!(
        counter("yog_indexer_event_insert_skipped_total"),
        Some(&DebugValue::Counter(1)),
        "a skipped insert must increment its own counter — otherwise the drop \
         is invisible again, which is the defect this PR corrects"
    );
    assert_eq!(
        counter("yog_indexer_instructions_indexed_total"),
        Some(&DebugValue::Counter(1)),
        "the event was still processed: rows written are indexed − skipped, and \
         that arithmetic breaks if a skip stops counting as indexed"
    );
}

// ── the ambiguity the ordering key cannot resolve is counted ─────────────────

/// Without `transaction_index` the order key cannot rank two transactions of one block: the
/// repository reports it, and this asserts the indexer counts it.
#[test]
fn a_same_slot_ambiguity_is_counted_on_the_applied_path() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(async {
                let calls: Calls = Arc::new(Mutex::new(Vec::new()));
                let p = build_with(
                    calls.clone(),
                    InsertOutcome::Inserted,
                    PoolCurrentStateUpsertOutcome {
                        applied: true,
                        same_slot_ambiguity: true,
                    },
                );
                p.persist(&MeteoraDammV2Event::Swap(swap())).await;
            });
    });

    // One snapshot: `Snapshotter::snapshot` swaps counters to 0.
    let snapshot = snapshotter.snapshot().into_vec();
    let counter = |name: &str| {
        snapshot
            .iter()
            .find(|(key, _, _, _)| key.key().name() == name)
            .map(|(_, _, _, value)| value)
    };

    assert_eq!(
        counter("yog_indexer_pool_current_state_same_slot_total"),
        Some(&DebugValue::Counter(1)),
        "an upsert the key could not rank must be counted even when it applied \
         — an ambiguity that wrongly accepts costs as much as one that wrongly \
         rejects"
    );
}

// ── a failed pool upsert is counted, not merely logged ───────────────────────

/// Mutations this is written against: dropping `record_pool_upsert_failure` from `discover_pool`,
/// or a label that diverges from `kind()`. Not `#[tokio::test]`: `with_local_recorder` binds the thread.
#[test]
fn a_failed_pool_upsert_is_counted_per_event_kind_and_the_event_still_persists() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let cases = [
        (
            MeteoraDammV2Event::Swap(swap()),
            &["pool:upsert", "insert:swap", "pcs:upsert"][..],
        ),
        (
            MeteoraDammV2Event::Liquidity(liquidity()),
            &["pool:upsert", "insert:liquidity", "pcs:upsert"][..],
        ),
        (
            MeteoraDammV2Event::InitializePool(initialize_pool(vec![0u8; 31])),
            &["pool:upsert", "insert:initialize_pool"][..],
        ),
    ];
    let kinds: Vec<&'static str> = cases.iter().map(|(event, _)| event.kind()).collect();

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(async {
                let calls: Calls = Arc::new(Mutex::new(Vec::new()));
                let p = build_with_failing_pool_upsert(calls.clone());
                for (event, recipe) in cases {
                    assert_eq!(route(&p, &calls, event).await, recipe);
                }
            });
    });

    // One snapshot: `Snapshotter::snapshot` swaps counters to 0.
    let snapshot = snapshotter.snapshot().into_vec();
    for kind in kinds {
        let counted = snapshot
            .iter()
            .find(|(key, _, _, _)| {
                key.key().name() == "yog_indexer_pool_upsert_failure_total"
                    && key
                        .key()
                        .labels()
                        .any(|l| l.key() == "event_kind" && l.value() == kind)
            })
            .map(|(_, _, _, value)| value);
        assert_eq!(
            counted,
            Some(&DebugValue::Counter(1)),
            "a failed pool upsert on a {kind} event must be counted, not only logged"
        );
    }
}

// ── fee parameters the decoder cannot read are counted, the event stored all the same ──────

/// Mutations this is written against: the insert skipped on a refusal, the counter dropped, a
/// readable blob counted, or a duplicate counted again. Not `#[tokio::test]`: `with_local_recorder`
/// binds the thread.
#[test]
fn undecodable_fee_parameters_are_counted_and_the_event_stored() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let update = |params_raw| {
        MeteoraDammV2Event::UpdatePoolFees(MeteoraDammV2UpdatePoolFeesEvent {
            pool_address: pk(1),
            signature: sg(),
            timestamp: ts(),
            slot: 1,
            transaction_index: None,
            event_index: 0,
            operator: pk(12),
            params_raw,
        })
    };

    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(async {
                let calls: Calls = Arc::new(Mutex::new(Vec::new()));
                let p = build(calls.clone());
                // Unreadable: a creation one byte short, an update cut inside its base fee.
                p.persist(&MeteoraDammV2Event::InitializePool(initialize_pool(vec![
                    0;
                    30
                ])))
                .await;
                p.persist(&update(vec![1, 160, 37])).await;
                // Readable: a constant 0.1 %, and the base fee set to 1 %.
                let constant = [vec![0x40, 0x42, 0x0f], vec![0; 28]].concat();
                p.persist(&MeteoraDammV2Event::InitializePool(initialize_pool(
                    constant,
                )))
                .await;
                p.persist(&update(vec![1, 0x80, 0x96, 0x98, 0, 0, 0, 0, 0, 0, 0]))
                    .await;

                let inserts: Vec<&str> = calls
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|call| call.starts_with("insert:"))
                    .copied()
                    .collect();
                assert_eq!(
                    inserts,
                    [
                        "insert:initialize_pool",
                        "insert:update_pool_fees",
                        "insert:initialize_pool",
                        "insert:update_pool_fees"
                    ],
                    "an event whose fees do not decode is stored raw like any other"
                );

                // A replay: the same unreadable blobs, their rows already there.
                let replay = build_with_fee_insert_outcome(calls.clone(), InsertOutcome::Skipped);
                replay
                    .persist(&MeteoraDammV2Event::InitializePool(initialize_pool(vec![
                        0;
                        30
                    ])))
                    .await;
                replay.persist(&update(vec![1, 160, 37])).await;
            });
    });

    // One snapshot: `Snapshotter::snapshot` swaps counters to 0.
    let mut counted: Vec<(String, String, u64)> = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter(|(key, _, _, _)| key.key().name() == "yog_indexer_fee_parameters_undecodable_total")
        .map(|(key, _, _, value)| {
            let label = |name: &str| {
                key.key()
                    .labels()
                    .find(|l| l.key() == name)
                    .map(|l| l.value().to_string())
                    .unwrap_or_default()
            };
            let DebugValue::Counter(n) = value else {
                panic!("a counter")
            };
            (label("event_kind"), label("reason"), n)
        })
        .collect();
    counted.sort_unstable();
    assert_eq!(
        counted,
        [
            ("initialize_pool".to_string(), "truncated".to_string(), 1),
            ("update_pool_fees".to_string(), "truncated".to_string(), 1),
        ],
        "each unreadable blob counted once, by event and reason: no readable one, no replay"
    );
}
