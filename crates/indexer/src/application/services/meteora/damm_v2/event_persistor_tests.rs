use super::*;
use async_trait::async_trait;
use chrono::DateTime;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use std::sync::Mutex;
use yog_core::RepositoryResult;
use yog_core::domain::{
    InsertOutcome, MeteoraDammV2LiquidityEventKind, MeteoraDammV2SplitAmounts,
    MeteoraDammV2SplitNumerators, MeteoraDammV2SplitPositionState, Pool,
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
/// Carries the outcome it should report, so a test can drive the
/// `Skipped` branch of `persist()` — the one the correction added.
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
    async fn mark_needs_refresh(&self, _: &Pubkey) -> RepositoryResult<()> {
        rec(&self.0, "pool:mark_needs_refresh");
        Ok(())
    }
    /// Never called from this crate: the registry's account-derived columns are
    /// written by yog-context alone. Present only to satisfy the trait.
    async fn set_registry_properties(
        &self,
        _: &Pubkey,
        _: &yog_core::domain::PoolRegistryProperties,
    ) -> RepositoryResult<()> {
        unreachable!("set_registry_properties belongs to yog-context, never the indexer")
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
    build_with_swap_outcome(calls, InsertOutcome::Inserted)
}

fn build_with_swap_outcome(
    calls: Calls,
    swap_outcome: InsertOutcome,
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
        initialize_pool: Arc::new(MockInit(calls.clone())),
        set_pool_status: Arc::new(MockSetStatus(calls.clone())),
        split_position: Arc::new(MockSplitPosition(calls.clone())),
        update_pool_fees: Arc::new(MockUpdateFees(calls.clone())),
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

    // initialize_pool: upsert + insert, NO projection and — since this crate
    // stopped writing pool properties — no fee decoding either. `discover_pool`
    // leaves the property columns NULL, which already queues the pool for
    // yog-context, so there is nothing to flag. The blob is still captured
    // verbatim on the event's own row.
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
                pool_fees_raw: fee_blob,
            })
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
                // The blob is no longer decoded at all: a fee change flags the
                // pool and yog-context re-reads the account.
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

// ---------------------------------------------------------------------------
// A skipped insert must be counted, not merely tolerated
// ---------------------------------------------------------------------------

/// The whole point of `InsertOutcome` is that a write which wrote nothing stops
/// passing for a success. That guarantee lives in one branch of `persist()`,
/// and until this test it was the only part of the correction with no test:
/// every mock reported `Inserted`, so deleting the branch left the suite green.
///
/// Asserts **both** counters, because their relationship is the contract:
/// `instructions_indexed` keeps meaning "events processed" and rows actually
/// written are `indexed − skipped`. A future change that stopped counting the
/// event as indexed would break that arithmetic silently.
///
/// Not `#[tokio::test]`: `with_local_recorder` installs the recorder on the
/// *current thread* for the duration of a closure, so the future has to be
/// driven inside it — hence the current-thread runtime.
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

                // The recipe still ran in full — a skip is not a failure.
                assert_eq!(
                    calls.lock().unwrap().clone(),
                    ["pool:upsert", "insert:swap", "pcs:upsert"]
                );
            });
    });

    // ONE snapshot, queried twice. `Snapshotter::snapshot` is destructive —
    // it reads counters with `swap(0)` — so a second call returns zeros and a
    // test written that way "proves" a counter that never fired.
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
