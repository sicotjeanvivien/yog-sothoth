//! Test support: fixtures and configurable repository mocks.
//!
//! Compiled only under `#[cfg(test)]`. Lives at the crate root so
//! every service test (PoolService today, others later) can share the
//! same mocks instead of redefining them per module.
//!
//! The mocks implement the read lenses the services actually consume
//! (`PoolCatalog`, `TokenPriceLookup`, `SignalFeed`, ...), so there are
//! no write-side stubs to drag along; a lens method a given test never
//! exercises is still `unreachable!()`. Each "once" mock yields
//! its preset value a single time (via `Mutex<Option<...>>`) because
//! `RepositoryResult` is not necessarily `Clone`. A second call panics
//! loudly rather than returning a misleading value.

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use std::collections::HashMap;
use std::sync::Mutex;

use yog_core::{
    Cursor, Page, PageDirection, PagePosition, PoolSort, RepositoryError, RepositoryResult,
    domain::{
        EventFreshnessRepository, GlobalAnalytics, GlobalAnalyticsRepository,
        MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventCursor,
        MeteoraDammV2LiquidityEventFeed, MeteoraDammV2LiquidityEventValued, MeteoraDammV2SwapEvent,
        MeteoraDammV2SwapEventCursor, MeteoraDammV2SwapEventFeed, NetworkStatus,
        NetworkStatusLookup, Pool, PoolAnalytics, PoolAnalyticsRepository, PoolCatalog, PoolCounts,
        PoolCurrentState, PoolCurrentStateLookup, PoolCursor, Protocol, Severity, Signal,
        SignalCursor, SignalFeed, SignalRecord, TokenMetadata, TokenMetadataLookup, TokenPrice,
        TokenPriceLookup,
    },
};

// ── Fixtures ───────────────────────────────────────────────────────

/// Deterministic pubkey from a single byte, for readable tests.
pub(crate) fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

pub(crate) fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

pub(crate) fn make_pool(addr: Pubkey, token_a: Pubkey, token_b: Pubkey) -> Pool {
    Pool {
        pool_address: addr,
        protocol: Protocol::MeteoraDammV2, // adapt to your variant name
        token_a_mint: Some(token_a),
        token_b_mint: Some(token_b),
        fee_bps: None,
        protocol_fee_percent: None,
        partner_fee_percent: None,
        referral_fee_percent: None,
        first_seen_at: ts(1_000),
        last_seen_at: ts(2_000),
    }
}

// NOTE: adapt the field set of TokenMetadata / TokenPrice to your real
// domain structs — only the Option-ness matters to the assertions.
pub(crate) fn make_metadata(mint: Pubkey, symbol: &str) -> TokenMetadata {
    TokenMetadata {
        mint,
        symbol: Some(symbol.to_string()),
        name: format!("{symbol} token").into(),
        decimals: 9,
        logo_uri: None,
        metadata_provider: yog_core::domain::MetadataProvider::HeliusDas,
        fetched_at: DateTime::from_timestamp_nanos(61_864_918_973_511),
        last_refresh_at: DateTime::from_timestamp_nanos(61_864_918_973_511),
    }
}

pub(crate) fn make_price(mint: Pubkey) -> TokenPrice {
    TokenPrice {
        mint,
        // adapt: rust_decimal::Decimal, source, fetched_at, etc.
        price_usd: rust_decimal::Decimal::new(100, 0),
        price_provider: yog_core::domain::PriceProvider::Helius,
        fetched_at: ts(1_500),
        confidence: Some(6.0),
    }
}

pub(crate) fn make_page(pools: Vec<Pool>, is_first: bool, is_last: bool) -> Page<Pool> {
    let prev = if is_first {
        None
    } else {
        pools.first().map(|p| {
            Cursor::Pool(PoolCursor {
                sort_column: yog_core::PoolSortColumn::FirstSeen,
                sort_value: ts(1_700_000_000),
                pool_address: p.pool_address,
            })
        })
    };
    let next = if is_last {
        None
    } else {
        pools.last().map(|p| {
            Cursor::Pool(PoolCursor {
                sort_column: yog_core::PoolSortColumn::FirstSeen,
                sort_value: ts(1_700_000_000),
                pool_address: p.pool_address,
            })
        })
    };
    Page {
        items: pools,
        next_cursor: next,
        prev_cursor: prev,
        is_first,
        is_last,
    }
}

fn take<T>(slot: &Mutex<Option<RepositoryResult<T>>>) -> RepositoryResult<T> {
    slot.lock()
        .unwrap()
        .take()
        .expect("mock method called more than once")
}

// ── Mock: PoolCatalog ────────────────────────────────────────────

pub(crate) struct PoolRepoOnce {
    paginated: Mutex<Option<RepositoryResult<Page<Pool>>>>,
    by_address: Mutex<Option<RepositoryResult<Option<Pool>>>>,
    by_addresses: Mutex<Option<RepositoryResult<Vec<Pool>>>>,
}

impl PoolRepoOnce {
    pub(crate) fn with_page(page: Page<Pool>) -> Self {
        Self {
            paginated: Mutex::new(Some(Ok(page))),
            by_address: Mutex::new(Some(Ok(None))),
            by_addresses: Mutex::new(Some(Ok(Vec::new()))),
        }
    }
    pub(crate) fn with_pool(pool: Option<Pool>) -> Self {
        Self {
            paginated: Mutex::new(Some(Ok(make_page(vec![], true, true)))),
            by_address: Mutex::new(Some(Ok(pool))),
            by_addresses: Mutex::new(Some(Ok(Vec::new()))),
        }
    }
    pub(crate) fn with_paginate_err() -> Self {
        Self {
            paginated: Mutex::new(Some(Err(RepositoryError::Integrity("boom".into())))),
            by_address: Mutex::new(Some(Ok(None))),
            by_addresses: Mutex::new(Some(Ok(Vec::new()))),
        }
    }
    /// Seed the batch `find_by_addresses` lookup — given intentionally
    /// unordered, so a top-N test can assert the service re-imposes the rank.
    pub(crate) fn with_pools(pools: Vec<Pool>) -> Self {
        Self {
            paginated: Mutex::new(Some(Ok(make_page(vec![], true, true)))),
            by_address: Mutex::new(Some(Ok(None))),
            by_addresses: Mutex::new(Some(Ok(pools))),
        }
    }
}

#[async_trait]
impl PoolCatalog for PoolRepoOnce {
    async fn find_by_address(&self, _addr: &Pubkey) -> RepositoryResult<Option<Pool>> {
        take(&self.by_address)
    }
    async fn counts(&self) -> RepositoryResult<yog_core::domain::PoolCounts> {
        unreachable!("counts not used by PoolService")
    }
    async fn find_by_addresses(&self, _addrs: &[Pubkey]) -> RepositoryResult<Vec<Pool>> {
        take(&self.by_addresses)
    }
    async fn find_paginated(
        &self,
        _cursor: Option<PoolCursor>,
        _direction: PageDirection,
        _position: Option<PagePosition>,
        _sort: PoolSort,
        _search: Option<String>,
        _limit: i64,
    ) -> RepositoryResult<Page<Pool>> {
        take(&self.paginated)
    }
}

// ── Mock: GlobalAnalyticsRepository ─────────────────────────────────

pub(crate) struct MockGlobalAnalyticsRepo {
    result: Mutex<Option<RepositoryResult<GlobalAnalytics>>>,
}

impl MockGlobalAnalyticsRepo {
    pub(crate) fn with(analytics: GlobalAnalytics) -> Self {
        Self {
            result: Mutex::new(Some(Ok(analytics))),
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            result: Mutex::new(Some(Err(RepositoryError::Integrity(
                "global analytics boom".into(),
            )))),
        }
    }
}

#[async_trait]
impl GlobalAnalyticsRepository for MockGlobalAnalyticsRepo {
    async fn global_analytics(&self) -> RepositoryResult<GlobalAnalytics> {
        take(&self.result)
    }
}

// ── Mock: PoolCatalog yielding only counts ───────────────────────

/// Minimal `PoolCatalog` mock for `StatsService`: only `counts()` is
/// exercised; every other method panics if reached.
pub(crate) struct PoolCountsRepo {
    counts: Mutex<Option<RepositoryResult<PoolCounts>>>,
}

impl PoolCountsRepo {
    pub(crate) fn with(counts: PoolCounts) -> Self {
        Self {
            counts: Mutex::new(Some(Ok(counts))),
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            counts: Mutex::new(Some(Err(RepositoryError::Integrity("counts boom".into())))),
        }
    }
}

#[async_trait]
impl PoolCatalog for PoolCountsRepo {
    async fn find_by_address(&self, _addr: &Pubkey) -> RepositoryResult<Option<Pool>> {
        unreachable!("find_by_address not used by StatsService")
    }
    async fn counts(&self) -> RepositoryResult<PoolCounts> {
        take(&self.counts)
    }
    async fn find_by_addresses(&self, _addrs: &[Pubkey]) -> RepositoryResult<Vec<Pool>> {
        unreachable!("find_by_addresses not used by StatsService")
    }
    async fn find_paginated(
        &self,
        _cursor: Option<PoolCursor>,
        _direction: PageDirection,
        _position: Option<PagePosition>,
        _sort: PoolSort,
        _search: Option<String>,
        _limit: i64,
    ) -> RepositoryResult<Page<Pool>> {
        unreachable!("find_paginated not used by StatsService")
    }
}

// ── Mock: PoolAnalyticsRepository ───────────────────────────────────

pub(crate) struct MockAnalyticsRepo {
    map: HashMap<Pubkey, PoolAnalytics>,
    /// Ranked addresses returned by `top_pool_addresses`, in order.
    top_ranked: Vec<Pubkey>,
    fail: bool,
}

impl MockAnalyticsRepo {
    pub(crate) fn with(map: HashMap<Pubkey, PoolAnalytics>) -> Self {
        Self {
            map,
            top_ranked: Vec::new(),
            fail: false,
        }
    }
    pub(crate) fn empty() -> Self {
        Self {
            map: HashMap::new(),
            top_ranked: Vec::new(),
            fail: false,
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            map: HashMap::new(),
            top_ranked: Vec::new(),
            fail: true,
        }
    }
    /// Set the ranking `top_pool_addresses` returns (already ordered).
    pub(crate) fn with_top(mut self, ranked: Vec<Pubkey>) -> Self {
        self.top_ranked = ranked;
        self
    }
}

#[async_trait]
impl PoolAnalyticsRepository for MockAnalyticsRepo {
    async fn batch_compute(
        &self,
        addresses: &[Pubkey],
    ) -> RepositoryResult<HashMap<Pubkey, PoolAnalytics>> {
        if self.fail {
            return Err(RepositoryError::Integrity("analytics boom".into()));
        }
        let mut out = HashMap::new();
        for a in addresses {
            if let Some(v) = self.map.get(a) {
                out.insert(*a, v.clone());
            }
        }
        Ok(out)
    }

    async fn history(
        &self,
        _pool_address: &Pubkey,
        _days: i32,
    ) -> RepositoryResult<Vec<yog_core::domain::PoolHistoryBucket>> {
        if self.fail {
            return Err(RepositoryError::Integrity("analytics boom".into()));
        }
        Ok(Vec::new())
    }

    async fn top_pool_addresses(
        &self,
        _metric: yog_core::domain::PoolRankMetric,
        limit: i64,
    ) -> RepositoryResult<Vec<Pubkey>> {
        if self.fail {
            return Err(RepositoryError::Integrity("analytics boom".into()));
        }
        Ok(self
            .top_ranked
            .iter()
            .take(limit.max(0) as usize)
            .copied()
            .collect())
    }
}

// ── Mock: TokenMetadataLookup ───────────────────────────────────

pub(crate) struct MockMetadataRepo {
    by_mint: HashMap<Pubkey, TokenMetadata>,
    fail: bool,
}

impl MockMetadataRepo {
    pub(crate) fn with(entries: Vec<(Pubkey, TokenMetadata)>) -> Self {
        Self {
            by_mint: entries.into_iter().collect(),
            fail: false,
        }
    }
    pub(crate) fn empty() -> Self {
        Self {
            by_mint: HashMap::new(),
            fail: false,
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            by_mint: HashMap::new(),
            fail: true,
        }
    }
}

#[async_trait]
impl TokenMetadataLookup for MockMetadataRepo {
    async fn find_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenMetadata>> {
        if self.fail {
            return Err(RepositoryError::Integrity("metadata boom".into()));
        }
        Ok(self.by_mint.get(mint).cloned())
    }
}

// ── Mock: TokenPriceLookup ──────────────────────────────────────

pub(crate) struct MockPriceRepo {
    by_mint: HashMap<Pubkey, TokenPrice>,
    fail: bool,
}

impl MockPriceRepo {
    pub(crate) fn with(entries: Vec<(Pubkey, TokenPrice)>) -> Self {
        Self {
            by_mint: entries.into_iter().collect(),
            fail: false,
        }
    }
    pub(crate) fn empty() -> Self {
        Self {
            by_mint: HashMap::new(),
            fail: false,
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            by_mint: HashMap::new(),
            fail: true,
        }
    }
}

#[async_trait]
impl TokenPriceLookup for MockPriceRepo {
    async fn find_latest_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenPrice>> {
        if self.fail {
            return Err(RepositoryError::Integrity("price boom".into()));
        }
        Ok(self.by_mint.get(mint).cloned())
    }
}

// ── Mock: PoolCurrentStateLookup ───────────────────────────────

pub(crate) struct MockPoolCurrentStateRepo {
    by_address: Mutex<Option<RepositoryResult<Option<PoolCurrentState>>>>,
}

impl MockPoolCurrentStateRepo {
    pub(crate) fn found(state: PoolCurrentState) -> Self {
        Self {
            by_address: Mutex::new(Some(Ok(Some(state)))),
        }
    }
    pub(crate) fn not_found() -> Self {
        Self {
            by_address: Mutex::new(Some(Ok(None))),
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            by_address: Mutex::new(Some(Err(RepositoryError::Integrity("state boom".into())))),
        }
    }
}

#[async_trait]
impl PoolCurrentStateLookup for MockPoolCurrentStateRepo {
    async fn get_by_address(&self, _: &str) -> RepositoryResult<Option<PoolCurrentState>> {
        take(&self.by_address)
    }
    async fn list_most_recent(
        &self,
        _limit: u32,
        _before: Option<DateTime<Utc>>,
    ) -> RepositoryResult<Vec<PoolCurrentState>> {
        unreachable!()
    }
}

// ── Mock: SwapEventRepository ────────────────────────────────────────

pub(crate) struct MockSwapEventRepo {
    find_paginated: Mutex<Option<RepositoryResult<Page<MeteoraDammV2SwapEvent>>>>,
}

impl MockSwapEventRepo {
    pub(crate) fn with_page(page: Page<MeteoraDammV2SwapEvent>) -> Self {
        Self {
            find_paginated: Mutex::new(Some(Ok(page))),
        }
    }
    pub(crate) fn empty() -> Self {
        Self::with_page(Page {
            items: vec![],
            next_cursor: None,
            prev_cursor: None,
            is_first: true,
            is_last: true,
        })
    }
    pub(crate) fn failing() -> Self {
        Self {
            find_paginated: Mutex::new(Some(Err(RepositoryError::Integrity("swap boom".into())))),
        }
    }
}

#[async_trait]
impl MeteoraDammV2SwapEventFeed for MockSwapEventRepo {
    async fn find_by_pool_paginated(
        &self,
        _pool_address: &Pubkey,
        _cursor: Option<MeteoraDammV2SwapEventCursor>,
        _direction: PageDirection,
        _position: Option<PagePosition>,
        _limit: i64,
    ) -> RepositoryResult<Page<MeteoraDammV2SwapEvent>> {
        take(&self.find_paginated)
    }
}

// ── Mock: LiquidityEventRepository ──────────────────────────────────

pub(crate) struct MockLiquidityEventRepo {
    find_paginated: Mutex<Option<RepositoryResult<Page<MeteoraDammV2LiquidityEventValued>>>>,
}

impl MockLiquidityEventRepo {
    pub(crate) fn with_page(page: Page<MeteoraDammV2LiquidityEventValued>) -> Self {
        Self {
            find_paginated: Mutex::new(Some(Ok(page))),
        }
    }
    pub(crate) fn empty() -> Self {
        Self::with_page(Page {
            items: vec![],
            next_cursor: None,
            prev_cursor: None,
            is_first: true,
            is_last: true,
        })
    }
    pub(crate) fn failing() -> Self {
        Self {
            find_paginated: Mutex::new(Some(Err(RepositoryError::Integrity("liq boom".into())))),
        }
    }
}

#[async_trait]
impl MeteoraDammV2LiquidityEventFeed for MockLiquidityEventRepo {
    async fn find_by_pool_paginated(
        &self,
        _pool_address: &Pubkey,
        _cursor: Option<MeteoraDammV2LiquidityEventCursor>,
        _direction: PageDirection,
        _position: Option<PagePosition>,
        _limit: i64,
    ) -> RepositoryResult<Page<MeteoraDammV2LiquidityEventValued>> {
        take(&self.find_paginated)
    }
}

// ── Mock: NetworkStatusLookup ───────────────────────────────────

pub(crate) struct MockNetworkStatusRepo {
    get: Mutex<Option<RepositoryResult<Option<NetworkStatus>>>>,
}

impl MockNetworkStatusRepo {
    pub(crate) fn found(status: NetworkStatus) -> Self {
        Self {
            get: Mutex::new(Some(Ok(Some(status)))),
        }
    }
    pub(crate) fn missing() -> Self {
        Self {
            get: Mutex::new(Some(Ok(None))),
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            get: Mutex::new(Some(Err(RepositoryError::Integrity("ns boom".into())))),
        }
    }
}

#[async_trait]
impl NetworkStatusLookup for MockNetworkStatusRepo {
    async fn get(&self) -> RepositoryResult<Option<NetworkStatus>> {
        take(&self.get)
    }
}

// ── Mock: EventFreshnessRepository ──────────────────────────────────

pub(crate) struct MockEventFreshnessRepo {
    last_event_at: Mutex<Option<RepositoryResult<Option<DateTime<Utc>>>>>,
}

impl MockEventFreshnessRepo {
    pub(crate) fn at(ts: DateTime<Utc>) -> Self {
        Self {
            last_event_at: Mutex::new(Some(Ok(Some(ts)))),
        }
    }
    pub(crate) fn never() -> Self {
        Self {
            last_event_at: Mutex::new(Some(Ok(None))),
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            last_event_at: Mutex::new(Some(Err(RepositoryError::Integrity("fresh boom".into())))),
        }
    }
}

#[async_trait]
impl EventFreshnessRepository for MockEventFreshnessRepo {
    async fn last_event_at(&self) -> RepositoryResult<Option<DateTime<Utc>>> {
        take(&self.last_event_at)
    }
}

// ── Additional fixtures ──────────────────────────────────────────────

pub(crate) fn make_swap_event(pool_address: Pubkey) -> MeteoraDammV2SwapEvent {
    use yog_core::domain::TradeDirection;

    MeteoraDammV2SwapEvent {
        pool_address,
        signature: sig_for_pool(pool_address, 1),
        timestamp: ts(1_500),
        trade_direction: TradeDirection::AtoB,
        amount_a: 1_000_000,
        amount_b: 2_000_000,
        reserve_a_after: 10_000_000,
        reserve_b_after: 20_000_000,
        next_sqrt_price: 1_000_000_000_000_000_000u128,
        claiming_fee: 0,
        protocol_fee: 100,
        compounding_fee: 50,
        referral_fee: 0,
        fee_token_is_a: true,
    }
}

pub(crate) fn make_liquidity_event(pool_address: Pubkey) -> MeteoraDammV2LiquidityEvent {
    use yog_core::domain::MeteoraDammV2LiquidityEventKind;

    MeteoraDammV2LiquidityEvent {
        pool_address,
        signature: sig_for_pool(pool_address, 1),
        timestamp: ts(1_600),
        liquidity_event_kind: MeteoraDammV2LiquidityEventKind::Add,
        amount_a: 5_000_000,
        amount_b: 10_000_000,
        reserve_a_after: 15_000_000,
        reserve_b_after: 30_000_000,
        liquidity_delta: 30_000_000,
        position: pool_address,
        owner: pool_address,
    }
}

pub(crate) fn make_network_status() -> NetworkStatus {
    NetworkStatus {
        slot: 300_000_000,
        rpc_latency_ms: 42,
        observed_at: ts(2_000),
    }
}

pub(crate) fn make_swap_page(
    events: Vec<MeteoraDammV2SwapEvent>,
    is_first: bool,
    is_last: bool,
) -> Page<MeteoraDammV2SwapEvent> {
    use yog_core::domain::MeteoraDammV2SwapEventCursor;

    let prev = if is_first {
        None
    } else {
        events.first().map(|e| {
            Cursor::MeteoraDammV2SwapEvent(MeteoraDammV2SwapEventCursor {
                timestamp: e.timestamp,
                signature: e.signature,
            })
        })
    };
    let next = if is_last {
        None
    } else {
        events.last().map(|e| {
            Cursor::MeteoraDammV2SwapEvent(MeteoraDammV2SwapEventCursor {
                timestamp: e.timestamp,
                signature: e.signature,
            })
        })
    };
    Page {
        items: events,
        next_cursor: next,
        prev_cursor: prev,
        is_first,
        is_last,
    }
}

pub(crate) fn make_liquidity_page(
    events: Vec<MeteoraDammV2LiquidityEvent>,
    is_first: bool,
    is_last: bool,
) -> Page<MeteoraDammV2LiquidityEventValued> {
    use yog_core::domain::MeteoraDammV2LiquidityEventCursor;

    let prev = if is_first {
        None
    } else {
        events.first().map(|e| {
            Cursor::MeteoraDammV2LiquidityEvent(MeteoraDammV2LiquidityEventCursor {
                timestamp: e.timestamp,
                signature: e.signature,
            })
        })
    };
    let next = if is_last {
        None
    } else {
        events.last().map(|e| {
            Cursor::MeteoraDammV2LiquidityEvent(MeteoraDammV2LiquidityEventCursor {
                timestamp: e.timestamp,
                signature: e.signature,
            })
        })
    };
    // The read path returns events wrapped with their USD value; the fixture
    // leaves it None (the service tests assert pagination, not valuation).
    let items = events
        .into_iter()
        .map(|event| MeteoraDammV2LiquidityEventValued {
            event,
            value_usd: None,
        })
        .collect();
    Page {
        items,
        next_cursor: next,
        prev_cursor: prev,
        is_first,
        is_last,
    }
}

pub(crate) fn make_pool_current_state(pool_address: Pubkey) -> PoolCurrentState {
    PoolCurrentState {
        pool_address,
        protocol: Protocol::MeteoraDammV2,
        reserve_a: 10_000_000,
        reserve_b: 20_000_000,
        last_sqrt_price: Some(1_000_000_000_000_000_000u128),
        liquidity: Some(500_000_000u128),
        last_swap_at: Some(ts(1_500)),
        last_liquidity_at: Some(ts(1_600)),
        last_event_at: ts(1_600),
        last_event_kind: yog_core::domain::LastEventKind::LiquidityAdd,
        last_signature: sig_for_pool(pool_address, 1),
        updated_at: ts(1_600),
    }
}

/// Build a deterministic, valid `Signature` for fixtures. The first 32
/// bytes come from the pool's pubkey so distinct pools get distinct
/// signatures; the last byte is a tag to distinguish event kinds for
/// the same pool (e.g. swap vs liquidity).
pub(crate) fn sig_for_pool(pool_address: Pubkey, tag: u8) -> Signature {
    let mut bytes = [0u8; 64];
    bytes[..32].copy_from_slice(&pool_address.to_bytes());
    bytes[63] = tag;
    Signature::from(bytes)
}

// ── Mock: SignalFeed ──────────────────────────────────────
//
// The api only ever sees the feed lens; the engine-side contract
// (insert_batch / latest_severity_by_pool) never reaches this crate,
// so the mock has nothing to stub out.

pub(crate) struct MockSignalRepo {
    list: Mutex<Option<RepositoryResult<Page<SignalRecord>>>>,
    latest_cursor: Mutex<Option<RepositoryResult<Option<SignalCursor>>>>,
    newer_than: Mutex<Option<RepositoryResult<Vec<SignalRecord>>>>,
}

impl MockSignalRepo {
    pub(crate) fn with_page(page: Page<SignalRecord>) -> Self {
        Self {
            list: Mutex::new(Some(Ok(page))),
            latest_cursor: Mutex::new(None),
            newer_than: Mutex::new(None),
        }
    }
    pub(crate) fn empty() -> Self {
        Self::with_page(Page::empty())
    }
    pub(crate) fn failing() -> Self {
        Self {
            list: Mutex::new(Some(Err(RepositoryError::Integrity("signal boom".into())))),
            latest_cursor: Mutex::new(None),
            newer_than: Mutex::new(None),
        }
    }
    /// Seed the streaming lens (poller tests): what `latest_cursor` and
    /// `newer_than` will yield, once each.
    pub(crate) fn feed(
        latest_cursor: RepositoryResult<Option<SignalCursor>>,
        newer_than: RepositoryResult<Vec<SignalRecord>>,
    ) -> Self {
        Self {
            list: Mutex::new(None),
            latest_cursor: Mutex::new(Some(latest_cursor)),
            newer_than: Mutex::new(Some(newer_than)),
        }
    }
}

#[async_trait]
impl SignalFeed for MockSignalRepo {
    async fn list(
        &self,
        _severity: Option<Severity>,
        _cursor: Option<SignalCursor>,
        _direction: PageDirection,
        _position: Option<PagePosition>,
        _limit: i64,
    ) -> RepositoryResult<Page<SignalRecord>> {
        take(&self.list)
    }
    async fn latest_cursor(&self) -> RepositoryResult<Option<SignalCursor>> {
        take(&self.latest_cursor)
    }
    async fn newer_than(
        &self,
        _after: &SignalCursor,
        _limit: i64,
    ) -> RepositoryResult<Vec<SignalRecord>> {
        take(&self.newer_than)
    }
}

pub(crate) fn make_signal_record(id: i64, pool_address: Pubkey) -> SignalRecord {
    SignalRecord {
        id,
        signal: Signal {
            detector: "flow_imbalance".to_string(),
            protocol: Protocol::MeteoraDammV2,
            pool_address,
            severity: Severity::Warning,
            value: rust_decimal::Decimal::new(75, 2),
            threshold: Some(rust_decimal::Decimal::new(6, 1)),
            message: Some("directional flow imbalance 0.75".to_string()),
            triggered_at: ts(1_700),
        },
    }
}

pub(crate) fn make_signal_page(records: Vec<SignalRecord>, is_last: bool) -> Page<SignalRecord> {
    let next = if is_last {
        None
    } else {
        records.last().map(|r| {
            Cursor::Signal(SignalCursor {
                triggered_at: r.signal.triggered_at,
                id: r.id,
            })
        })
    };
    Page {
        items: records,
        next_cursor: next,
        prev_cursor: None,
        is_first: true,
        is_last,
    }
}
