//! Test support: fixtures and configurable repository mocks.
//!
//! Compiled only under `#[cfg(test)]`. Lives at the crate root so
//! every service test (PoolService today, others later) can share the
//! same mocks instead of redefining them per module.
//!
//! The mocks deliberately implement only what the services exercise;
//! unused trait methods are `unreachable!()`. Each "once" mock yields
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
        EventFreshnessRepository, LiquidityCursor, LiquidityEvent, LiquidityEventRepository,
        NetworkStatus, NetworkStatusRepository, Pool, PoolAnalytics, PoolAnalyticsRepository,
        PoolCurrentState, PoolCurrentStateRepository, PoolCurrentStateUpsert, PoolCursor,
        PoolRepository, Protocol, SwapCursor, SwapEvent, SwapEventRepository, TokenMetadata,
        TokenMetadataRepository, TokenPrice, TokenPriceRepository,
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
        token_a_mint: token_a,
        token_b_mint: token_b,
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
        metadata_source: "Helius".to_string(),
        fetched_at: DateTime::from_timestamp_nanos(61_864_918_973_511),
        last_refresh_at: DateTime::from_timestamp_nanos(61_864_918_973_511),
    }
}

pub(crate) fn make_price(mint: Pubkey) -> TokenPrice {
    TokenPrice {
        mint,
        // adapt: rust_decimal::Decimal, source, fetched_at, etc.
        price_usd: rust_decimal::Decimal::new(100, 0),
        price_source: yog_core::domain::PriceSource::Helius,
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

// ── Mock: PoolRepository ────────────────────────────────────────────

pub(crate) struct PoolRepoOnce {
    paginated: Mutex<Option<RepositoryResult<Page<Pool>>>>,
    by_address: Mutex<Option<RepositoryResult<Option<Pool>>>>,
}

impl PoolRepoOnce {
    pub(crate) fn with_page(page: Page<Pool>) -> Self {
        Self {
            paginated: Mutex::new(Some(Ok(page))),
            by_address: Mutex::new(Some(Ok(None))),
        }
    }
    pub(crate) fn with_pool(pool: Option<Pool>) -> Self {
        Self {
            paginated: Mutex::new(Some(Ok(make_page(vec![], true, true)))),
            by_address: Mutex::new(Some(Ok(pool))),
        }
    }
    pub(crate) fn with_paginate_err() -> Self {
        Self {
            paginated: Mutex::new(Some(Err(RepositoryError::Integrity("boom".into())))),
            by_address: Mutex::new(Some(Ok(None))),
        }
    }
}

#[async_trait]
impl PoolRepository for PoolRepoOnce {
    async fn upsert(&self, _pool: &Pool) -> RepositoryResult<()> {
        unreachable!("upsert not used by PoolService")
    }
    async fn touch_last_seen(&self, _addr: &Pubkey) -> RepositoryResult<()> {
        unreachable!("touch_last_seen not used by PoolService")
    }
    async fn find_by_address(&self, _addr: &Pubkey) -> RepositoryResult<Option<Pool>> {
        take(&self.by_address)
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

// ── Mock: PoolAnalyticsRepository ───────────────────────────────────

pub(crate) struct MockAnalyticsRepo {
    map: HashMap<Pubkey, PoolAnalytics>,
    fail: bool,
}

impl MockAnalyticsRepo {
    pub(crate) fn with(map: HashMap<Pubkey, PoolAnalytics>) -> Self {
        Self { map, fail: false }
    }
    pub(crate) fn empty() -> Self {
        Self {
            map: HashMap::new(),
            fail: false,
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            map: HashMap::new(),
            fail: true,
        }
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
}

// ── Mock: TokenMetadataRepository ───────────────────────────────────

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
impl TokenMetadataRepository for MockMetadataRepo {
    async fn upsert(&self, _m: &TokenMetadata) -> RepositoryResult<()> {
        unreachable!()
    }
    async fn list_known_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
        unreachable!()
    }
    async fn list_missing_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
        unreachable!()
    }
    async fn find_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenMetadata>> {
        if self.fail {
            return Err(RepositoryError::Integrity("metadata boom".into()));
        }
        Ok(self.by_mint.get(mint).cloned())
    }
}

// ── Mock: TokenPriceRepository ──────────────────────────────────────

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
impl TokenPriceRepository for MockPriceRepo {
    async fn insert_batch(&self, _p: &[TokenPrice]) -> RepositoryResult<()> {
        unreachable!()
    }
    async fn find_latest_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenPrice>> {
        if self.fail {
            return Err(RepositoryError::Integrity("price boom".into()));
        }
        Ok(self.by_mint.get(mint).cloned())
    }
}

// ── Mock: PoolCurrentStateRepository ───────────────────────────────

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
impl PoolCurrentStateRepository for MockPoolCurrentStateRepo {
    async fn upsert(&self, _: &PoolCurrentStateUpsert) -> RepositoryResult<bool> {
        unreachable!("upsert not used by api services")
    }
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
    find_paginated: Mutex<Option<RepositoryResult<Page<SwapEvent>>>>,
}

impl MockSwapEventRepo {
    pub(crate) fn with_page(page: Page<SwapEvent>) -> Self {
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
impl SwapEventRepository for MockSwapEventRepo {
    async fn insert(&self, _: &SwapEvent) -> RepositoryResult<()> {
        unreachable!()
    }
    async fn find_by_pool_paginated(
        &self,
        _pool_address: &Pubkey,
        _cursor: Option<SwapCursor>,
        _direction: PageDirection,
        _position: Option<PagePosition>,
        _limit: i64,
    ) -> RepositoryResult<Page<SwapEvent>> {
        take(&self.find_paginated)
    }
}

// ── Mock: LiquidityEventRepository ──────────────────────────────────

pub(crate) struct MockLiquidityEventRepo {
    find_paginated: Mutex<Option<RepositoryResult<Page<LiquidityEvent>>>>,
}

impl MockLiquidityEventRepo {
    pub(crate) fn with_page(page: Page<LiquidityEvent>) -> Self {
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
impl LiquidityEventRepository for MockLiquidityEventRepo {
    async fn insert(&self, _: &LiquidityEvent) -> RepositoryResult<()> {
        unreachable!()
    }
    async fn find_by_pool_paginated(
        &self,
        _pool_address: &Pubkey,
        _cursor: Option<LiquidityCursor>,
        _direction: PageDirection,
        _position: Option<PagePosition>,
        _limit: i64,
    ) -> RepositoryResult<Page<LiquidityEvent>> {
        take(&self.find_paginated)
    }
}

// ── Mock: NetworkStatusRepository ───────────────────────────────────

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
impl NetworkStatusRepository for MockNetworkStatusRepo {
    async fn upsert(&self, _: &NetworkStatus) -> RepositoryResult<()> {
        unreachable!()
    }
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

pub(crate) fn make_swap_event(pool_address: Pubkey) -> SwapEvent {
    use yog_core::domain::TradeDirection;

    SwapEvent {
        pool_address,
        protocol: Protocol::MeteoraDammV2,
        signature: sig_for_pool(pool_address, 1),
        timestamp: ts(1_500),
        token_a_mint: pk(10),
        token_b_mint: pk(11),
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

pub(crate) fn make_liquidity_event(pool_address: Pubkey) -> LiquidityEvent {
    use yog_core::domain::LiquidityEventKind;

    LiquidityEvent {
        pool_address,
        protocol: Protocol::MeteoraDammV2,
        signature: sig_for_pool(pool_address, 1),
        timestamp: ts(1_600),
        token_a_mint: pk(10),
        token_b_mint: pk(11),
        liquidity_event_kind: LiquidityEventKind::Add,
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
    events: Vec<SwapEvent>,
    is_first: bool,
    is_last: bool,
) -> Page<SwapEvent> {
    use yog_core::domain::SwapCursor;

    let prev = if is_first {
        None
    } else {
        events.first().map(|e| {
            Cursor::Swap(SwapCursor {
                timestamp: e.timestamp,
                signature: e.signature.clone(),
            })
        })
    };
    let next = if is_last {
        None
    } else {
        events.last().map(|e| {
            Cursor::Swap(SwapCursor {
                timestamp: e.timestamp,
                signature: e.signature.clone(),
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
    events: Vec<LiquidityEvent>,
    is_first: bool,
    is_last: bool,
) -> Page<LiquidityEvent> {
    use yog_core::domain::LiquidityCursor;

    let prev = if is_first {
        None
    } else {
        events.first().map(|e| {
            Cursor::Liquidity(LiquidityCursor {
                timestamp: e.timestamp,
                signature: e.signature.clone(),
            })
        })
    };
    let next = if is_last {
        None
    } else {
        events.last().map(|e| {
            Cursor::Liquidity(LiquidityCursor {
                timestamp: e.timestamp,
                signature: e.signature.clone(),
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

pub(crate) fn make_pool_current_state(pool_address: Pubkey) -> PoolCurrentState {
    PoolCurrentState {
        pool_address: pool_address,
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
