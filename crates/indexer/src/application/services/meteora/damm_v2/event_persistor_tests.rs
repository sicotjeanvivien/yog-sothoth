use super::*;
use async_trait::async_trait;
use chrono::DateTime;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use std::sync::Mutex;
use yog_core::RepositoryResult;
use yog_core::domain::{
    MeteoraDammV2LiquidityEventKind, Pool, PoolCurrentStateRepository, PoolCurrentStateUpsert,
    PoolRepository, TradeDirection,
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
    async fn set_fee_config(&self, _: &Pubkey, _: &str, _: bool) -> RepositoryResult<()> {
        rec(&self.0, "pool:set_fee_config");
        Ok(())
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
        create_position: Arc::new(MockCreate(calls.clone())),
        close_position: Arc::new(MockClose(calls.clone())),
        lock_position: Arc::new(MockLock(calls.clone())),
        permanent_lock_position: Arc::new(MockPermLock(calls.clone())),
        initialize_pool: Arc::new(MockInit(calls.clone())),
        set_pool_status: Arc::new(MockSetStatus(calls.clone())),
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
                params_raw: vec![1, 160, 37, 38, 0, 0, 0, 0, 0],
            })
        )
        .await,
        ["pool:touch", "pool:set_fee_bps", "insert:update_pool_fees"]
    );
}
