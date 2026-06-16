//! Meteora DAMM v2 event persistor.
//!
//! Owns the per-event-kind DAMM v2 repositories and dispatches each
//! event to its persistence recipe. Pool registry upserts and
//! pool_current_state projection are delegated to the shared
//! [`PoolMaintenance`] helper.

use std::sync::Arc;
use std::time::Instant;
use tracing::{error, warn};
use yog_core::domain::{
    MeteoraDammV2ClaimPositionFeeEvent, MeteoraDammV2ClaimPositionFeeEventRepository,
    MeteoraDammV2ClaimRewardEvent, MeteoraDammV2ClaimRewardEventRepository,
    MeteoraDammV2ClosePositionEvent, MeteoraDammV2ClosePositionEventRepository,
    MeteoraDammV2CreatePositionEvent, MeteoraDammV2CreatePositionEventRepository,
    MeteoraDammV2Event, MeteoraDammV2InitializePoolEvent,
    MeteoraDammV2InitializePoolEventRepository, MeteoraDammV2LiquidityEvent,
    MeteoraDammV2LiquidityEventRepository, MeteoraDammV2LockPositionEvent,
    MeteoraDammV2LockPositionEventRepository, MeteoraDammV2PermanentLockPositionEvent,
    MeteoraDammV2PermanentLockPositionEventRepository, MeteoraDammV2SetPoolStatusEvent,
    MeteoraDammV2SetPoolStatusEventRepository, MeteoraDammV2SwapEvent,
    MeteoraDammV2SwapEventRepository, MeteoraDammV2UpdatePoolFeesEvent,
    MeteoraDammV2UpdatePoolFeesEventRepository, Protocol,
};

use crate::application::services::{EventPersistorMetrics, PoolMaintenance};

/// The per-event-kind DAMM v2 repositories, bundled so the persistor takes a
/// single named-field argument instead of one positional `Arc` per event kind
/// (which grows unbounded as new events are added).
pub(crate) struct DammV2Repos {
    pub swap_event: Arc<dyn MeteoraDammV2SwapEventRepository>,
    pub liquidity_event: Arc<dyn MeteoraDammV2LiquidityEventRepository>,
    pub claim_position_fee: Arc<dyn MeteoraDammV2ClaimPositionFeeEventRepository>,
    pub claim_reward: Arc<dyn MeteoraDammV2ClaimRewardEventRepository>,
    pub create_position: Arc<dyn MeteoraDammV2CreatePositionEventRepository>,
    pub close_position: Arc<dyn MeteoraDammV2ClosePositionEventRepository>,
    pub lock_position: Arc<dyn MeteoraDammV2LockPositionEventRepository>,
    pub permanent_lock_position: Arc<dyn MeteoraDammV2PermanentLockPositionEventRepository>,
    pub initialize_pool: Arc<dyn MeteoraDammV2InitializePoolEventRepository>,
    pub set_pool_status: Arc<dyn MeteoraDammV2SetPoolStatusEventRepository>,
    pub update_pool_fees: Arc<dyn MeteoraDammV2UpdatePoolFeesEventRepository>,
}

pub(crate) struct MeteoraDammV2EventPersistor {
    repos: DammV2Repos,
    pool_maintenance: Arc<PoolMaintenance>,
}

impl MeteoraDammV2EventPersistor {
    const PROTOCOL: Protocol = Protocol::MeteoraDammV2;

    pub(crate) fn new(repos: DammV2Repos, pool_maintenance: Arc<PoolMaintenance>) -> Self {
        Self {
            repos,
            pool_maintenance,
        }
    }

    /// Apply a Meteora DAMM v2 event to its persistence targets.
    ///
    /// Per-event failures are logged and counted, never propagated —
    /// the caller continues with the next event.
    pub(crate) async fn persist(&self, event: &MeteoraDammV2Event) {
        let kind = event.kind();
        let start = Instant::now();

        let result = match event {
            MeteoraDammV2Event::Swap(e) => self.persist_swap(e).await,
            MeteoraDammV2Event::Liquidity(e) => self.persist_liquidity(e).await,
            MeteoraDammV2Event::ClaimPositionFee(e) => self.persist_claim_position_fee(e).await,
            MeteoraDammV2Event::ClaimReward(e) => self.persist_claim_reward(e).await,
            MeteoraDammV2Event::CreatePosition(e) => self.persist_create_position(e).await,
            MeteoraDammV2Event::ClosePosition(e) => self.persist_close_position(e).await,
            MeteoraDammV2Event::LockPosition(e) => self.persist_lock_position(e).await,
            MeteoraDammV2Event::PermanentLockPosition(e) => {
                self.persist_permanent_lock_position(e).await
            }
            MeteoraDammV2Event::InitializePool(e) => self.persist_initialize_pool(e).await,
            MeteoraDammV2Event::SetPoolStatus(e) => self.persist_set_pool_status(e).await,
            MeteoraDammV2Event::UpdatePoolFees(e) => self.persist_update_pool_fees(e).await,
        };

        let elapsed = start.elapsed().as_secs_f64();
        EventPersistorMetrics::record_persist_duration(&Self::PROTOCOL, kind, elapsed);

        match result {
            Ok(()) => {
                EventPersistorMetrics::record_indexed(&Self::PROTOCOL, kind);
            }
            Err(err) => {
                error!(
                    protocol = %Self::PROTOCOL.as_str(),
                    kind,
                    error = %err,
                    "persist event failed"
                );
                EventPersistorMetrics::record_persist_failure(&Self::PROTOCOL, kind);
            }
        }
    }

    async fn persist_swap(&self, event: &MeteoraDammV2SwapEvent) -> anyhow::Result<()> {
        if let Err(err) = self
            .pool_maintenance
            .discover_pool(Self::PROTOCOL, event.pool_address)
            .await
        {
            warn!(error = %err, kind = "swap", "pool upsert failed");
        }
        let insert_result = self
            .repos
            .swap_event
            .insert(event)
            .await
            .map_err(anyhow::Error::new);
        if insert_result.is_ok() {
            self.pool_maintenance
                .update_pool_current_state_from_swap(Self::PROTOCOL, event)
                .await;
        }
        insert_result
    }

    async fn persist_liquidity(&self, event: &MeteoraDammV2LiquidityEvent) -> anyhow::Result<()> {
        if let Err(err) = self
            .pool_maintenance
            .discover_pool(Self::PROTOCOL, event.pool_address)
            .await
        {
            warn!(error = %err, kind = "liquidity", "pool upsert failed");
        }
        let insert_result = self
            .repos
            .liquidity_event
            .insert(event)
            .await
            .map_err(anyhow::Error::new);
        if insert_result.is_ok() {
            self.pool_maintenance
                .update_pool_current_state_from_liquidity(Self::PROTOCOL, event)
                .await;
        }
        insert_result
    }

    async fn persist_claim_position_fee(
        &self,
        event: &MeteoraDammV2ClaimPositionFeeEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .claim_position_fee
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_claim_reward(
        &self,
        event: &MeteoraDammV2ClaimRewardEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .claim_reward
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// A create-position event carries no mints or reserves, so it neither
    /// upserts the pool registry nor updates the current-state projection —
    /// it only refreshes the pool's last-seen marker and records the event.
    async fn persist_create_position(
        &self,
        event: &MeteoraDammV2CreatePositionEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .create_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Same recipe as create-position: no mints/reserves, so just refresh the
    /// pool's last-seen marker and record the event.
    async fn persist_close_position(
        &self,
        event: &MeteoraDammV2ClosePositionEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .close_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Lock carries vesting params but no pool reserves — same touch_pool +
    /// insert recipe as the other lifecycle events.
    async fn persist_lock_position(
        &self,
        event: &MeteoraDammV2LockPositionEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .lock_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_permanent_lock_position(
        &self,
        event: &MeteoraDammV2PermanentLockPositionEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .permanent_lock_position
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    /// Pool genesis carries both mints, so it registers the pool authoritatively
    /// (full upsert) rather than just touching last-seen. It does not feed the
    /// current-state projection — there is no price/reserve trajectory yet.
    ///
    /// The InitializePool event does carry the mints, but the `pools` registry
    /// gets them from a single source — yog-context decoding the pool account —
    /// to avoid a dual-write that could disagree on A/B ordering. So here we
    /// only register the pool; the mints land on the event's own table.
    async fn persist_initialize_pool(
        &self,
        event: &MeteoraDammV2InitializePoolEvent,
    ) -> anyhow::Result<()> {
        if let Err(err) = self
            .pool_maintenance
            .discover_pool(Self::PROTOCOL, event.pool_address)
            .await
        {
            warn!(error = %err, kind = "initialize_pool", "pool upsert failed");
        }
        // Decode the headline base fee from the genesis "voie C" blob and record
        // it as a pool property. Skip-and-log on an undecodable blob (unknown
        // fee mode): the pool keeps a NULL fee_bps rather than a wrong value.
        match yog_core::amm::damm_v2::decode_base_fee_bps(&event.pool_fees_raw) {
            Ok(fee_bps) => {
                self.pool_maintenance
                    .set_fee_bps(Self::PROTOCOL, &event.pool_address, fee_bps)
                    .await;
            }
            Err(err) => {
                warn!(error = %err, kind = "initialize_pool", "fee_bps decode failed");
            }
        }
        self.repos
            .initialize_pool
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_set_pool_status(
        &self,
        event: &MeteoraDammV2SetPoolStatusEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        self.repos
            .set_pool_status
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }

    async fn persist_update_pool_fees(
        &self,
        event: &MeteoraDammV2UpdatePoolFeesEvent,
    ) -> anyhow::Result<()> {
        self.pool_maintenance
            .touch_pool(Self::PROTOCOL, &event.pool_address)
            .await;
        // An operator fee change refreshes the pool's headline fee tier — but
        // only when it actually touched the base fee (Some). Skip-and-log on an
        // undecodable blob: keep the previous fee_bps rather than a wrong value.
        match yog_core::amm::damm_v2::decode_updated_base_fee_bps(&event.params_raw) {
            Ok(Some(fee_bps)) => {
                self.pool_maintenance
                    .set_fee_bps(Self::PROTOCOL, &event.pool_address, fee_bps)
                    .await;
            }
            Ok(None) => {}
            Err(err) => {
                warn!(error = %err, kind = "update_pool_fees", "fee_bps decode failed");
            }
        }
        self.repos
            .update_pool_fees
            .insert(event)
            .await
            .map_err(anyhow::Error::new)
    }
}

// ---------------------------------------------------------------------------
// Tests — dispatch + recipe routing
// ---------------------------------------------------------------------------
//
// Verify that `persist()` routes each `MeteoraDammV2Event` variant to the
// correct per-event-kind repository AND applies the right pool-maintenance
// recipe: full upsert for swap/liquidity/genesis, last-seen touch otherwise,
// and the current-state projection only for swap/liquidity. All repositories
// (and the pool/projection repos behind `PoolMaintenance`) are mocked to
// append to an ordered call log — no database is involved.
#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::DateTime;
    use solana_pubkey::Pubkey;
    use solana_signature::Signature;
    use std::sync::Mutex;
    use yog_core::domain::{
        MeteoraDammV2LiquidityEventCursor, MeteoraDammV2LiquidityEventKind,
        MeteoraDammV2SwapEventCursor, Pool, PoolCurrentState, PoolCurrentStateRepository,
        PoolCurrentStateUpsert, PoolCursor, PoolRepository, TradeDirection,
    };
    use yog_core::{Page, PageDirection, PagePosition, PoolSort, RepositoryResult};

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

    // Ring-1 repos: record `insert`; their read methods are never hit by
    // `persist()`, so they stub out.
    struct MockSwap(Calls);
    #[async_trait]
    impl MeteoraDammV2SwapEventRepository for MockSwap {
        async fn insert(&self, _e: &MeteoraDammV2SwapEvent) -> RepositoryResult<()> {
            rec(&self.0, "insert:swap");
            Ok(())
        }
        async fn find_by_pool_paginated(
            &self,
            _: &Pubkey,
            _: Option<MeteoraDammV2SwapEventCursor>,
            _: PageDirection,
            _: Option<PagePosition>,
            _: i64,
        ) -> RepositoryResult<Page<MeteoraDammV2SwapEvent>> {
            unimplemented!("not exercised by persist()")
        }
    }
    struct MockLiquidity(Calls);
    #[async_trait]
    impl MeteoraDammV2LiquidityEventRepository for MockLiquidity {
        async fn insert(&self, _e: &MeteoraDammV2LiquidityEvent) -> RepositoryResult<()> {
            rec(&self.0, "insert:liquidity");
            Ok(())
        }
        async fn find_by_pool_paginated(
            &self,
            _: &Pubkey,
            _: Option<MeteoraDammV2LiquidityEventCursor>,
            _: PageDirection,
            _: Option<PagePosition>,
            _: i64,
        ) -> RepositoryResult<Page<MeteoraDammV2LiquidityEvent>> {
            unimplemented!("not exercised by persist()")
        }
    }
    struct MockClaimFee(Calls);
    #[async_trait]
    impl MeteoraDammV2ClaimPositionFeeEventRepository for MockClaimFee {
        async fn insert(&self, _e: &MeteoraDammV2ClaimPositionFeeEvent) -> RepositoryResult<()> {
            rec(&self.0, "insert:claim_position_fee");
            Ok(())
        }
        async fn find_by_pool(
            &self,
            _: &Pubkey,
            _: i64,
        ) -> RepositoryResult<Vec<MeteoraDammV2ClaimPositionFeeEvent>> {
            unimplemented!("not exercised by persist()")
        }
    }
    struct MockClaimReward(Calls);
    #[async_trait]
    impl MeteoraDammV2ClaimRewardEventRepository for MockClaimReward {
        async fn insert(&self, _e: &MeteoraDammV2ClaimRewardEvent) -> RepositoryResult<()> {
            rec(&self.0, "insert:claim_reward");
            Ok(())
        }
        async fn find_by_pool(
            &self,
            _: &Pubkey,
            _: i64,
        ) -> RepositoryResult<Vec<MeteoraDammV2ClaimRewardEvent>> {
            unimplemented!("not exercised by persist()")
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
        async fn find_by_address(&self, _: &Pubkey) -> RepositoryResult<Option<Pool>> {
            unimplemented!("not exercised by persist()")
        }
        async fn find_paginated(
            &self,
            _: Option<PoolCursor>,
            _: PageDirection,
            _: Option<PagePosition>,
            _: PoolSort,
            _: Option<String>,
            _: i64,
        ) -> RepositoryResult<Page<Pool>> {
            unimplemented!("not exercised by persist()")
        }
    }
    struct MockPcsRepo(Calls);
    #[async_trait]
    impl PoolCurrentStateRepository for MockPcsRepo {
        async fn upsert(&self, _: &PoolCurrentStateUpsert) -> RepositoryResult<bool> {
            rec(&self.0, "pcs:upsert");
            Ok(true)
        }
        async fn get_by_address(&self, _: &str) -> RepositoryResult<Option<PoolCurrentState>> {
            unimplemented!("not exercised by persist()")
        }
        async fn list_most_recent(
            &self,
            _: u32,
            _: Option<DateTime<chrono::Utc>>,
        ) -> RepositoryResult<Vec<PoolCurrentState>> {
            unimplemented!("not exercised by persist()")
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
                MeteoraDammV2Event::PermanentLockPosition(
                    MeteoraDammV2PermanentLockPositionEvent {
                        pool_address: pk(1),
                        signature: sg(),
                        timestamp: ts(),
                        position: pk(4),
                        lock_liquidity_amount: 1,
                        total_permanent_locked_liquidity: 1,
                    }
                )
            )
            .await,
            ["pool:touch", "insert:permanent_lock_position"]
        );

        // initialize_pool: full upsert + decode/record fee + insert, NO
        // projection. The 27-byte fee blob (numerator 2_500_000, mode 0)
        // decodes cleanly, so the fee_bps step fires between upsert and insert.
        let mut fee_blob = vec![0u8; 27];
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
            ["pool:upsert", "pool:set_fee_bps", "insert:initialize_pool"]
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
}
