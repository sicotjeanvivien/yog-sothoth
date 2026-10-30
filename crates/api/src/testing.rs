//! Test support: fixtures and repository mocks shared by every service test. The mocks
//! implement only the read lenses the services consume; a "once" mock yields its value a
//! single time, and a second call panics.

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use solana_pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use yog_persistence::{PgHealthChecker, PgPool};

use yog_core::{
    Cursor, Page, PageDirection, PagePosition, RepositoryError, RepositoryResult,
    domain::{
        Announcement, AnnouncementCursor, AnnouncementEntry, AnnouncementKind, AnnouncementLookup,
        AnnouncementSeverity, AnnouncementStatus, ClaimKind, ClaimTotals, ClaimTotalsLookup,
        DiscoveredTvlLookup, EventCursor, EventFreshnessRepository, EventScope, FrozenHours,
        FrozenPeriod, LiquidityChangeEvent, LiquidityChangeEventLookup, LiquidityFlows,
        LiquidityMove, LiquidityMoveCursor, LiquidityMoveList, LiquidityMoveLookup,
        LiquidityMoveOrder, LiquidityTotals, MeteoraDammV2PoolProperties,
        MeteoraDammV2PoolPropertiesLookup, NetworkStatus, NetworkStatusLookup, Pool, PoolCatalog,
        PoolChangeEvent, PoolChangeEventLookup, PoolCreationEvent, PoolCreationEventLookup,
        PoolCreations, PoolCreationsLookup, PoolCursor, PoolEntry, PoolScope, PoolSwapBucket,
        PositionChangeCounts, PositionChangeCountsLookup, PositionChangeEvent,
        PositionChangeEventLookup, PositionFeeClaimEvent, PositionFeeClaimEventLookup, Protocol,
        ProtocolActivityLookup, ProtocolBucket, ProtocolDiscoveredTvl, ProtocolFeeClaimEvent,
        ProtocolFeeClaimEventLookup, ProtocolPoolCreations, ProtocolPools, ProtocolSwapActivity,
        ProtocolTvl, ProtocolTvlChange, ProtocolTvlLookup, RewardChangeEvent,
        RewardChangeEventLookup, RewardClaimEvent, RewardClaimEventLookup, RotationRun,
        RotationRunCursor, RotationRunLookup, RotationRunOrder, RotationRunScope,
        RotationRunTotals, RunSplit, Severity, Signal, SignalCursor, SignalFeed, SignalFilter,
        SignalRecord, SwapActivity, SwapActivityLookup, SwapEvent, SwapEventLookup,
        TokenActivityCursor, TokenActivityFeed, TokenActivityPage, TokenCursor, TokenEntry,
        TokenMetadata, TokenMetadataLookup, TokenPrice, TokenPriceCursor, TokenPriceLookup,
        TranchePeriod, Window, WrittenTranches,
    },
};

use crate::application::{
    AnnouncementService, ClaimTotalsService, DiscoveredTvlService, IngestionService,
    LiquidityChangeEventService, LiquidityMoveService, NetworkService, PoolChangeEventService,
    PoolCreationEventService, PoolCreationsService, PoolService, PositionChangeCountsService,
    PositionChangeEventService, PositionFeeClaimEventService, ProtocolCatalogService,
    ProtocolFeeClaimEventService, ProtocolTvlService, RewardChangeEventService,
    RewardClaimEventService, RotationRunService, SignalService, SwapActivityService,
    SwapEventService, TokenService, WindowsService, WorkSlots,
};
use crate::http::{AppState, build_router};
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use tower::ServiceExt;

/// A state for one request through the router: an empty signal feed, no known token, every
/// other read failing. ⚠️ Its mocks answer once; its database never answers, so `/readyz`
/// waits out its probe.
pub(crate) fn app_state() -> AppState {
    let work_slots = WorkSlots::new(1, Duration::from_secs(1));
    AppState {
        network_service: Arc::new(NetworkService::new(Arc::new(
            MockNetworkStatusRepo::failing(),
        ))),
        windows_service: Arc::new(WindowsService::new(frozen_hours_repo(None))),
        ingestion_service: Arc::new(IngestionService::new(Arc::new(
            MockEventFreshnessRepo::failing(),
        ))),
        signal_service: Arc::new(SignalService::new(
            Arc::new(MockSignalRepo::empty()),
            Arc::new(PoolRepoOnce::with_pools(vec![])),
            Arc::new(MockMetadataRepo::empty()),
        )),
        signal_stream: broadcast::channel(1).0,
        stream_slots: Arc::new(Semaphore::new(1)),
        protocol_catalog_service: Arc::new(ProtocolCatalogService::new(
            Arc::new(PoolCountsRepo::failing()),
            vec![],
            work_slots.clone(),
        )),
        swap_activity_service: Arc::new(SwapActivityService::new(
            frozen_hours_repo(None),
            Arc::new(MockSwapActivityRepo::default()),
            Arc::new(MockTokenActivityRepo::default()),
        )),
        pool_creations_service: Arc::new(PoolCreationsService::new(
            frozen_hours_repo(None),
            Arc::new(MockPoolCreationsRepo::default()),
        )),
        liquidity_move_service: Arc::new(LiquidityMoveService::new(
            frozen_hours_repo(None),
            Arc::new(MockLiquidityMoveRepo::default()),
        )),
        claim_totals_service: Arc::new(ClaimTotalsService::new(
            frozen_hours_repo(None),
            Arc::new(MockClaimTotalsRepo::default()),
        )),
        position_change_counts_service: Arc::new(PositionChangeCountsService::new(
            frozen_hours_repo(None),
            Arc::new(MockPositionChangeCountsRepo::default()),
        )),
        rotation_run_service: Arc::new(RotationRunService::new(
            frozen_hours_repo(None),
            Arc::new(MockRotationRunRepo::default()),
        )),
        protocol_tvl_service: Arc::new(ProtocolTvlService::new(
            frozen_hours_repo(None),
            Arc::new(MockProtocolTvlRepo::default()),
        )),
        discovered_tvl_service: Arc::new(DiscoveredTvlService::new(
            frozen_hours_repo(None),
            Arc::new(MockDiscoveredTvlRepo::default()),
        )),
        swap_event_service: Arc::new(SwapEventService::new(
            Arc::new(MockSwapEventRepo::default()),
        )),
        liquidity_change_event_service: Arc::new(LiquidityChangeEventService::new(Arc::new(
            MockLiquidityChangeEventRepo::default(),
        ))),
        pool_creation_event_service: Arc::new(PoolCreationEventService::new(Arc::new(
            MockPoolCreationEventRepo::default(),
        ))),
        position_change_event_service: Arc::new(PositionChangeEventService::new(Arc::new(
            MockPositionChangeEventRepo::default(),
        ))),
        reward_change_event_service: Arc::new(RewardChangeEventService::new(Arc::new(
            MockRewardChangeEventRepo::default(),
        ))),
        pool_change_event_service: Arc::new(PoolChangeEventService::new(Arc::new(
            MockPoolChangeEventRepo::default(),
        ))),
        position_fee_claim_event_service: Arc::new(PositionFeeClaimEventService::new(Arc::new(
            MockPositionFeeClaimEventRepo::default(),
        ))),
        protocol_fee_claim_event_service: Arc::new(ProtocolFeeClaimEventService::new(Arc::new(
            MockProtocolFeeClaimEventRepo::default(),
        ))),
        reward_claim_event_service: Arc::new(RewardClaimEventService::new(Arc::new(
            MockRewardClaimEventRepo::default(),
        ))),
        pool_service: Arc::new(PoolService::new(
            Arc::new(PoolRepoOnce::with_pools(vec![])),
            Arc::new(MockDammV2PropertiesRepo::default()),
        )),
        work_slots,
        token_service: Arc::new(TokenService::new(
            Arc::new(MockMetadataRepo::empty()),
            Arc::new(MockPriceRepo::empty()),
        )),
        announcement_service: Arc::new(AnnouncementService::new(Arc::new(MockAnnouncementRepo))),
        health_checker: Arc::new(PgHealthChecker::new(
            PgPool::connect_lazy("postgres://unused@127.0.0.1:1/unused")
                .expect("a well-formed URL"),
        )),
        shutdown: CancellationToken::new(),
    }
}

/// `method uri` through the whole router on [`app_state`]: the status, the headers, and the
/// `detail` of a Problem Details body (else the body as text).
pub(crate) async fn call(method: Method, uri: &str) -> (StatusCode, HeaderMap, String) {
    call_on(app_state(), method, uri).await
}

/// [`call`] on `state`.
pub(crate) async fn call_on(
    state: AppState,
    method: Method,
    uri: &str,
) -> (StatusCode, HeaderMap, String) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let response = build_router(state, vec![]).oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let detail = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|json| json["detail"].as_str().map(str::to_string))
        .unwrap_or_else(|| String::from_utf8_lossy(&body).into_owned());
    (parts.status, parts.headers, detail)
}

/// [`call`] with `GET`, its `Content-Type` read out — empty when there is none.
pub(crate) async fn get(uri: &str) -> (StatusCode, String, String) {
    let (status, headers, detail) = call(Method::GET, uri).await;
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    (status, content_type, detail)
}

/// Deterministic pubkey from a single byte, for readable tests.
pub(crate) fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

pub(crate) fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

/// The string a wire enum serialises to. Each `*_matches_the_domain` test
/// compares it with the domain's `as_str()`: red when a wire variant is renamed,
/// or loses its `rename`, without its domain string.
pub(crate) fn wire_tag(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .expect("a wire enum serialises")
        .as_str()
        .expect("a wire enum is a JSON string")
        .to_string()
}

pub(crate) fn make_pool(addr: Pubkey, token_a: Pubkey, token_b: Pubkey) -> Pool {
    Pool {
        pool_address: addr,
        protocol: Protocol::MeteoraDammV2, // adapt to your variant name
        token_a_mint: Some(token_a),
        token_b_mint: Some(token_b),
        fee_bps: None,
        first_seen_at: ts(1_000),
        last_seen_at: ts(2_000),
    }
}

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
        price_usd: rust_decimal::Decimal::new(100, 0),
        price_provider: yog_core::domain::PriceProvider::Helius,
        fetched_at: ts(1_500),
        confidence: Some(6.0),
    }
}

fn take<T>(slot: &Mutex<Option<RepositoryResult<T>>>) -> RepositoryResult<T> {
    slot.lock()
        .unwrap()
        .take()
        .expect("mock method called more than once")
}

pub(crate) struct PoolRepoOnce {
    by_addresses: Mutex<Option<RepositoryResult<Vec<Pool>>>>,
}

impl PoolRepoOnce {
    /// Seeds `find_by_addresses`, unordered.
    pub(crate) fn with_pools(pools: Vec<Pool>) -> Self {
        Self {
            by_addresses: Mutex::new(Some(Ok(pools))),
        }
    }
}

#[async_trait]
impl PoolCatalog for PoolRepoOnce {
    async fn counts_by_protocol(&self) -> RepositoryResult<Vec<ProtocolPools>> {
        unreachable!("counts_by_protocol not used by SignalService")
    }
    async fn find_by_addresses(&self, _addrs: &[Pubkey]) -> RepositoryResult<Vec<Pool>> {
        take(&self.by_addresses)
    }
    async fn list_entries(
        &self,
        _: PoolScope,
        _: Option<PoolCursor>,
        _: i64,
    ) -> RepositoryResult<Page<PoolEntry>> {
        Ok(Page::empty())
    }
    async fn find_entry(&self, _: &Pubkey) -> RepositoryResult<Option<PoolEntry>> {
        Ok(None)
    }
}

/// A `PoolCatalog` for `PoolService::get_pool`: it knows `entries` and answers them by address.
#[derive(Default)]
pub(crate) struct MockPoolEntriesRepo {
    pub(crate) entries: Vec<PoolEntry>,
}

#[async_trait]
impl PoolCatalog for MockPoolEntriesRepo {
    async fn counts_by_protocol(&self) -> RepositoryResult<Vec<ProtocolPools>> {
        unreachable!("counts_by_protocol not used by PoolService")
    }
    async fn find_by_addresses(&self, _: &[Pubkey]) -> RepositoryResult<Vec<Pool>> {
        unreachable!("find_by_addresses not used by PoolService::get_pool")
    }
    async fn list_entries(
        &self,
        _: PoolScope,
        _: Option<PoolCursor>,
        _: i64,
    ) -> RepositoryResult<Page<PoolEntry>> {
        unreachable!("list_entries not used by PoolService::get_pool")
    }
    async fn find_entry(&self, address: &Pubkey) -> RepositoryResult<Option<PoolEntry>> {
        Ok(self
            .entries
            .iter()
            .find(|entry| entry.pool.pool_address == *address)
            .cloned())
    }
}

/// A DAMM v2 satellite that knows `properties` and records each pool read.
#[derive(Default)]
pub(crate) struct MockDammV2PropertiesRepo {
    pub(crate) properties: Vec<MeteoraDammV2PoolProperties>,
    pub(crate) reads: Mutex<Vec<Pubkey>>,
}

#[async_trait]
impl MeteoraDammV2PoolPropertiesLookup for MockDammV2PropertiesRepo {
    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
    ) -> RepositoryResult<Option<MeteoraDammV2PoolProperties>> {
        self.reads.lock().unwrap().push(*pool_address);
        Ok(self
            .properties
            .iter()
            .find(|p| p.pool_address == *pool_address)
            .cloned())
    }
}

/// A `PoolCatalog` for `ProtocolCatalogService`: only the counts by protocol answer.
pub(crate) struct PoolCountsRepo {
    by_protocol: Mutex<Option<RepositoryResult<Vec<ProtocolPools>>>>,
}

impl PoolCountsRepo {
    pub(crate) fn by_protocol(counts: Vec<ProtocolPools>) -> Self {
        Self {
            by_protocol: Mutex::new(Some(Ok(counts))),
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            by_protocol: Mutex::new(Some(Err(RepositoryError::Integrity("counts boom".into())))),
        }
    }
}

#[async_trait]
impl PoolCatalog for PoolCountsRepo {
    async fn counts_by_protocol(&self) -> RepositoryResult<Vec<ProtocolPools>> {
        take(&self.by_protocol)
    }
    async fn find_by_addresses(&self, _addrs: &[Pubkey]) -> RepositoryResult<Vec<Pool>> {
        unreachable!("find_by_addresses not used by ProtocolCatalogService")
    }
    async fn list_entries(
        &self,
        _: PoolScope,
        _: Option<PoolCursor>,
        _: i64,
    ) -> RepositoryResult<Page<PoolEntry>> {
        unreachable!("list_entries not used by ProtocolCatalogService")
    }
    async fn find_entry(&self, _: &Pubkey) -> RepositoryResult<Option<PoolEntry>> {
        unreachable!("find_entry not used by ProtocolCatalogService")
    }
}

/// A `ProtocolActivityLookup` that answers `frozen` and `tranches` every time.
pub(crate) struct MockProtocolActivityRepo {
    frozen: Option<FrozenHours>,
    tranches: Option<WrittenTranches>,
}

impl MockProtocolActivityRepo {
    pub(crate) fn new(frozen: Option<FrozenHours>) -> Self {
        Self {
            frozen,
            tranches: None,
        }
    }

    pub(crate) fn with_tranches(tranches: WrittenTranches) -> Self {
        Self {
            frozen: None,
            tranches: Some(tranches),
        }
    }

    /// Frozen hours and written tranches both: the frozen windows and the last hour.
    pub(crate) fn with_hours_and_tranches(frozen: FrozenHours, tranches: WrittenTranches) -> Self {
        Self {
            frozen: Some(frozen),
            tranches: Some(tranches),
        }
    }
}

#[async_trait]
impl ProtocolActivityLookup for MockProtocolActivityRepo {
    async fn frozen_hours(&self) -> RepositoryResult<Option<FrozenHours>> {
        Ok(self.frozen)
    }
    async fn written_tranches(&self) -> RepositoryResult<Option<WrittenTranches>> {
        Ok(self.tranches)
    }
}

/// A `TokenActivityFeed` whose last ranking was written at `ranked_at`, `None`
/// for none: it records each window read and answers an empty page of it.
#[derive(Default)]
pub(crate) struct MockTokenActivityRepo {
    pub(crate) ranked_at: Option<DateTime<Utc>>,
    pub(crate) reads: Mutex<Vec<Window>>,
    pub(crate) tranche_reads: Mutex<Vec<TranchePeriod>>,
}

#[async_trait]
impl TokenActivityFeed for MockTokenActivityRepo {
    async fn list(
        &self,
        window: Window,
        _cursor: Option<TokenActivityCursor>,
        _limit: i64,
    ) -> RepositoryResult<Option<TokenActivityPage>> {
        self.reads.lock().unwrap().push(window);
        Ok(self.ranked_at.map(|last_hour| TokenActivityPage {
            last_hour,
            page: Page::empty(),
        }))
    }
    async fn list_in_tranches(
        &self,
        period: &TranchePeriod,
        _cursor: Option<TokenActivityCursor>,
        _limit: i64,
    ) -> RepositoryResult<TokenActivityPage> {
        self.tranche_reads.lock().unwrap().push(*period);
        Ok(TokenActivityPage {
            last_hour: period.as_of,
            page: Page::empty(),
        })
    }
}

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
    async fn list_entries(
        &self,
        _: Option<TokenCursor>,
        _: i64,
    ) -> RepositoryResult<Page<TokenEntry>> {
        Ok(Page::empty())
    }
}

pub(crate) struct MockPriceRepo {
    by_mint: HashMap<Pubkey, TokenPrice>,
    fail: bool,
    /// The start of each window `list_prices` was asked about.
    pub(crate) windows_from: Mutex<Vec<DateTime<Utc>>>,
}

impl MockPriceRepo {
    pub(crate) fn with(entries: Vec<(Pubkey, TokenPrice)>) -> Self {
        Self {
            by_mint: entries.into_iter().collect(),
            fail: false,
            windows_from: Mutex::new(vec![]),
        }
    }
    pub(crate) fn empty() -> Self {
        Self {
            by_mint: HashMap::new(),
            fail: false,
            windows_from: Mutex::new(vec![]),
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            by_mint: HashMap::new(),
            fail: true,
            windows_from: Mutex::new(vec![]),
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
    async fn list_prices(
        &self,
        _: &Pubkey,
        from: DateTime<Utc>,
        _: Option<TokenPriceCursor>,
        _: i64,
    ) -> RepositoryResult<Page<TokenPrice>> {
        self.windows_from.lock().unwrap().push(from);
        Ok(Page::empty())
    }
}

/// An `AnnouncementLookup` that holds no announcement of its own.
pub(crate) struct MockAnnouncementRepo;

#[async_trait]
impl AnnouncementLookup for MockAnnouncementRepo {
    /// One announcement of the status asked for, none without one: the status echoed back.
    async fn list_entries(
        &self,
        status: Option<AnnouncementStatus>,
        _: DateTime<Utc>,
        _: Option<AnnouncementCursor>,
        _: i64,
    ) -> RepositoryResult<Page<AnnouncementEntry>> {
        let Some(status) = status else {
            return Ok(Page::empty());
        };
        Ok(Page {
            items: vec![AnnouncementEntry {
                announcement: Announcement {
                    id: 1,
                    kind: AnnouncementKind::Release,
                    severity: AnnouncementSeverity::Info,
                    message: "m".to_owned(),
                    link_url: None,
                    starts_at: ts(1_000),
                    ends_at: None,
                },
                status,
            }],
            ..Page::empty()
        })
    }
}

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

pub(crate) fn make_network_status() -> NetworkStatus {
    NetworkStatus {
        slot: 300_000_000,
        rpc_latency_ms: 42,
        observed_at: ts(2_000),
    }
}

pub(crate) struct MockSignalRepo {
    list: Mutex<Option<RepositoryResult<Page<SignalRecord>>>>,
    latest_cursor: Mutex<Option<RepositoryResult<Option<SignalCursor>>>>,
    newer_than: Mutex<Option<RepositoryResult<Vec<SignalRecord>>>>,
    /// The filter of each `list`.
    pub(crate) filters: Mutex<Vec<SignalFilter>>,
}

impl MockSignalRepo {
    pub(crate) fn with_page(page: Page<SignalRecord>) -> Self {
        Self {
            list: Mutex::new(Some(Ok(page))),
            latest_cursor: Mutex::new(None),
            newer_than: Mutex::new(None),
            filters: Mutex::new(vec![]),
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
            filters: Mutex::new(vec![]),
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
            filters: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl SignalFeed for MockSignalRepo {
    async fn list(
        &self,
        filter: SignalFilter,
        _cursor: Option<SignalCursor>,
        _direction: PageDirection,
        _position: Option<PagePosition>,
        _limit: i64,
    ) -> RepositoryResult<Page<SignalRecord>> {
        self.filters.lock().unwrap().push(filter);
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

/// Slots for tests that do not exercise them: plenty, and a short wait.
pub(crate) fn work_slots() -> crate::application::WorkSlots {
    crate::application::WorkSlots::new(8, std::time::Duration::from_millis(100))
}

type PeriodRead = (&'static str, FrozenPeriod, Option<Protocol>);
type InstantRead = (&'static str, DateTime<Utc>, Option<Protocol>);

/// A day of the month of the frozen hours of [`make_frozen_hours`].
pub(crate) fn at_october(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap()
}

/// Facts from the 13th at 13:00 through the 20th at 12:00: 24h and 7d are covered, 30d is not.
pub(crate) fn make_frozen_hours() -> FrozenHours {
    FrozenHours {
        first: at_october(13, 13),
        last_frozen: at_october(20, 12),
    }
}

/// The frozen hours `frozen` answers, the only read the produced-resource services make of it.
pub(crate) fn frozen_hours_repo(frozen: Option<FrozenHours>) -> Arc<MockProtocolActivityRepo> {
    Arc::new(MockProtocolActivityRepo::new(frozen))
}

/// A `SwapActivityLookup` that answers nothing and records the period and the scope of each read.
#[derive(Default)]
pub(crate) struct MockSwapActivityRepo {
    pub(crate) widths: Mutex<Vec<TimeDelta>>,
    pub(crate) reads: Mutex<Vec<PeriodRead>>,
    pub(crate) tranche_reads: Mutex<Vec<(&'static str, TranchePeriod, Option<Protocol>)>>,
}

impl MockSwapActivityRepo {
    fn record(&self, name: &'static str, period: &FrozenPeriod, protocol: Option<Protocol>) {
        self.reads.lock().unwrap().push((name, *period, protocol));
    }
}

#[async_trait]
impl SwapActivityLookup for MockSwapActivityRepo {
    async fn find_protocol_totals(
        &self,
        period: &FrozenPeriod,
        _: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolSwapActivity>> {
        self.record("swap_totals", period, protocol);
        Ok(vec![])
    }
    async fn list_protocol_hourly(
        &self,
        period: &FrozenPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<SwapActivity>>> {
        self.record("swap_hourly", period, protocol);
        Ok(vec![])
    }
    async fn list_protocol_buckets(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<SwapActivity>>> {
        self.widths.lock().unwrap().push(width);
        self.record("swap_buckets", period, protocol);
        Ok(vec![])
    }
    async fn find_protocol_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolSwapActivity>> {
        let read = ("swap_totals", *period, protocol);
        self.tranche_reads.lock().unwrap().push(read);
        Ok(vec![])
    }
    async fn list_protocol_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<SwapActivity>>> {
        let read = ("swap_tranches", *period, protocol);
        self.tranche_reads.lock().unwrap().push(read);
        Ok(vec![])
    }
    async fn find_pool_totals(
        &self,
        period: &FrozenPeriod,
        _: Pubkey,
    ) -> RepositoryResult<Option<SwapActivity>> {
        self.record("pool_swap_totals", period, None);
        Ok(None)
    }
    async fn list_pool_hourly(
        &self,
        period: &FrozenPeriod,
        _: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>> {
        self.record("pool_swap_hourly", period, None);
        Ok(vec![])
    }
    async fn list_pool_buckets(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        _: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>> {
        self.widths.lock().unwrap().push(width);
        self.record("pool_swap_buckets", period, None);
        Ok(vec![])
    }
    async fn find_pool_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        _: Pubkey,
    ) -> RepositoryResult<Option<SwapActivity>> {
        let read = ("pool_swap_totals", *period, None);
        self.tranche_reads.lock().unwrap().push(read);
        Ok(None)
    }
    async fn list_pool_tranches(
        &self,
        period: &TranchePeriod,
        _: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>> {
        let read = ("pool_swap_tranches", *period, None);
        self.tranche_reads.lock().unwrap().push(read);
        Ok(vec![])
    }
}

/// A `PoolCreationsLookup` that answers nothing and records the width of each bucket read.
#[derive(Default)]
pub(crate) struct MockPoolCreationsRepo {
    pub(crate) widths: Mutex<Vec<TimeDelta>>,
}

#[async_trait]
impl PoolCreationsLookup for MockPoolCreationsRepo {
    async fn list_totals(
        &self,
        _: &FrozenPeriod,
        _: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolPoolCreations>> {
        Ok(vec![])
    }
    async fn list_hourly(
        &self,
        _: &FrozenPeriod,
        _: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<PoolCreations>>> {
        Ok(vec![])
    }
    async fn list_buckets(
        &self,
        _: &FrozenPeriod,
        width: TimeDelta,
        _: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<PoolCreations>>> {
        self.widths.lock().unwrap().push(width);
        Ok(vec![])
    }
    async fn list_totals_in_tranches(
        &self,
        _: &TranchePeriod,
        _: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolPoolCreations>> {
        Ok(vec![])
    }
    async fn list_tranches(
        &self,
        _: &TranchePeriod,
        _: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<PoolCreations>>> {
        Ok(vec![])
    }
}

/// A `PositionChangeCountsLookup` answering zero changes, which records each period it reads.
#[derive(Default)]
pub(crate) struct MockPositionChangeCountsRepo {
    pub(crate) widths: Mutex<Vec<TimeDelta>>,
    pub(crate) reads: Mutex<Vec<FrozenPeriod>>,
    pub(crate) series_reads: Mutex<Vec<&'static str>>,
    pub(crate) tranche_reads: Mutex<Vec<TranchePeriod>>,
}

#[async_trait]
impl PositionChangeCountsLookup for MockPositionChangeCountsRepo {
    async fn find_totals(
        &self,
        period: &FrozenPeriod,
        _: Protocol,
    ) -> RepositoryResult<Option<PositionChangeCounts>> {
        self.reads.lock().unwrap().push(*period);
        Ok(Some(PositionChangeCounts {
            opened: 0,
            vesting_locks: 0,
            permanent_locks: 0,
            splits: 0,
            closed: 0,
        }))
    }
    async fn find_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        _: Protocol,
    ) -> RepositoryResult<Option<PositionChangeCounts>> {
        self.tranche_reads.lock().unwrap().push(*period);
        Ok(None)
    }
    async fn list_hourly(
        &self,
        _: &FrozenPeriod,
        _: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<PositionChangeCounts>>> {
        self.series_reads.lock().unwrap().push("position_hourly");
        Ok(vec![])
    }
    async fn list_buckets(
        &self,
        _: &FrozenPeriod,
        width: TimeDelta,
        _: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<PositionChangeCounts>>> {
        self.widths.lock().unwrap().push(width);
        self.series_reads.lock().unwrap().push("position_buckets");
        Ok(vec![])
    }
    async fn list_tranches(
        &self,
        period: &TranchePeriod,
        _: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<PositionChangeCounts>>> {
        self.series_reads.lock().unwrap().push("position_tranches");
        self.tranche_reads.lock().unwrap().push(*period);
        Ok(vec![])
    }
}

/// A `ClaimTotalsLookup` measured from `since`, answering zero claims, which records the period
/// and the protocol of each read.
#[derive(Default)]
pub(crate) struct MockClaimTotalsRepo {
    pub(crate) widths: Mutex<Vec<TimeDelta>>,
    pub(crate) since: Option<DateTime<Utc>>,
    pub(crate) reads: Mutex<Vec<PeriodRead>>,
    pub(crate) tranche_reads: Mutex<Vec<(&'static str, ClaimKind, TranchePeriod)>>,
}

impl MockClaimTotalsRepo {
    fn totals(&self, name: &'static str, period: &FrozenPeriod, protocol: Protocol) -> ClaimTotals {
        self.reads
            .lock()
            .unwrap()
            .push((name, *period, Some(protocol)));
        ClaimTotals {
            claims: 0,
            claims_valued: 0,
            amount_usd: rust_decimal::Decimal::ZERO,
            owners: 0,
        }
    }
}

#[async_trait]
impl ClaimTotalsLookup for MockClaimTotalsRepo {
    async fn find_measured_since(&self, _: Protocol) -> RepositoryResult<Option<DateTime<Utc>>> {
        Ok(self.since)
    }
    async fn find_position_fee_claim_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals> {
        Ok(self.totals("position_fee_claim_totals", period, protocol))
    }
    async fn find_reward_claim_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals> {
        Ok(self.totals("reward_claim_totals", period, protocol))
    }
    async fn find_totals_in_tranches(
        &self,
        kind: ClaimKind,
        period: &TranchePeriod,
        _: Protocol,
    ) -> RepositoryResult<ClaimTotals> {
        let read = ("claim_totals", kind, *period);
        self.tranche_reads.lock().unwrap().push(read);
        Ok(ClaimTotals {
            claims: 0,
            claims_valued: 0,
            amount_usd: rust_decimal::Decimal::ZERO,
            owners: 0,
        })
    }
    async fn list_hourly(
        &self,
        kind: ClaimKind,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<ClaimTotals>>> {
        let name = match kind {
            ClaimKind::PositionFee => "position_fee_claim_hourly",
            ClaimKind::Reward => "reward_claim_hourly",
        };
        self.reads
            .lock()
            .unwrap()
            .push((name, *period, Some(protocol)));
        Ok(vec![])
    }
    async fn list_buckets(
        &self,
        kind: ClaimKind,
        period: &FrozenPeriod,
        width: TimeDelta,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<ClaimTotals>>> {
        self.widths.lock().unwrap().push(width);
        let name = match kind {
            ClaimKind::PositionFee => "position_fee_claim_buckets",
            ClaimKind::Reward => "reward_claim_buckets",
        };
        self.reads
            .lock()
            .unwrap()
            .push((name, *period, Some(protocol)));
        Ok(vec![])
    }
    async fn list_tranches(
        &self,
        kind: ClaimKind,
        period: &TranchePeriod,
        _: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<ClaimTotals>>> {
        let read = ("claim_tranches", kind, *period);
        self.tranche_reads.lock().unwrap().push(read);
        Ok(vec![])
    }
}

/// A `LiquidityMoveLookup` measured from `since`, answering nothing, which records the period
/// and the scope of each read.
#[derive(Default)]
pub(crate) struct MockLiquidityMoveRepo {
    pub(crate) widths: Mutex<Vec<TimeDelta>>,
    pub(crate) since: Option<DateTime<Utc>>,
    pub(crate) reads: Mutex<Vec<PeriodRead>>,
    pub(crate) tranche_reads: Mutex<Vec<(&'static str, TranchePeriod)>>,
}

impl MockLiquidityMoveRepo {
    fn record(&self, name: &'static str, period: &FrozenPeriod, protocol: Option<Protocol>) {
        self.reads.lock().unwrap().push((name, *period, protocol));
    }
}

#[async_trait]
impl LiquidityMoveLookup for MockLiquidityMoveRepo {
    async fn find_measured_since(&self, _: Protocol) -> RepositoryResult<Option<DateTime<Utc>>> {
        Ok(self.since)
    }
    async fn find_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Option<LiquidityTotals>> {
        self.record("liquidity_totals", period, Some(protocol));
        Ok(None)
    }
    async fn list_hourly(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<LiquidityFlows>>> {
        self.record("liquidity_hourly", period, Some(protocol));
        Ok(vec![])
    }
    async fn list_buckets(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<LiquidityFlows>>> {
        self.widths.lock().unwrap().push(width);
        self.record("liquidity_buckets", period, Some(protocol));
        Ok(vec![])
    }
    async fn find_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        _: Protocol,
    ) -> RepositoryResult<LiquidityTotals> {
        self.tranche_reads
            .lock()
            .unwrap()
            .push(("liquidity_totals", *period));
        Ok(LiquidityTotals {
            flows: LiquidityFlows {
                created_usd: rust_decimal::Decimal::ZERO,
                added_usd: rust_decimal::Decimal::ZERO,
                removed_usd: rust_decimal::Decimal::ZERO,
                moves: 0,
                moves_valued: 0,
            },
            owners: 0,
        })
    }
    async fn list_tranches(
        &self,
        period: &TranchePeriod,
        _: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<LiquidityFlows>>> {
        self.tranche_reads
            .lock()
            .unwrap()
            .push(("liquidity_tranches", *period));
        Ok(vec![])
    }
    async fn list_moves(
        &self,
        period: &FrozenPeriod,
        _: Window,
        _: LiquidityMoveList,
        _: LiquidityMoveOrder,
        _: Option<LiquidityMoveCursor>,
        _: i64,
    ) -> RepositoryResult<Page<LiquidityMove>> {
        self.record("moves", period, None);
        Ok(Page::empty())
    }
    async fn list_moves_in_tranches(
        &self,
        period: &TranchePeriod,
        _: Window,
        _: LiquidityMoveList,
        _: LiquidityMoveOrder,
        _: Option<LiquidityMoveCursor>,
        _: i64,
    ) -> RepositoryResult<Page<LiquidityMove>> {
        self.tranche_reads
            .lock()
            .unwrap()
            .push(("moves_in_tranches", *period));
        Ok(Page::empty())
    }
}

/// A `ProtocolTvlLookup` that answers nothing and records the instant and the scope of each read.
#[derive(Default)]
pub(crate) struct MockProtocolTvlRepo {
    pub(crate) instants: Mutex<Vec<InstantRead>>,
    pub(crate) windows: Mutex<Vec<Window>>,
}

#[async_trait]
impl ProtocolTvlLookup for MockProtocolTvlRepo {
    async fn list_latest(
        &self,
        last: DateTime<Utc>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvl>> {
        self.instants.lock().unwrap().push(("tvl", last, protocol));
        Ok(vec![])
    }
    async fn list_hourly(
        &self,
        first: DateTime<Utc>,
        _: DateTime<Utc>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvl>> {
        self.instants
            .lock()
            .unwrap()
            .push(("tvl_hourly", first, protocol));
        Ok(vec![])
    }
    async fn list_changes(
        &self,
        last: DateTime<Utc>,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvlChange>> {
        self.windows.lock().unwrap().push(window);
        self.instants
            .lock()
            .unwrap()
            .push(("tvl_changes", last, protocol));
        Ok(vec![])
    }
}

/// A `DiscoveredTvlLookup` that answers nothing and records the instant and the scope of each
/// read.
#[derive(Default)]
pub(crate) struct MockDiscoveredTvlRepo {
    pub(crate) instants: Mutex<Vec<InstantRead>>,
}

#[async_trait]
impl DiscoveredTvlLookup for MockDiscoveredTvlRepo {
    async fn list_totals(
        &self,
        last: DateTime<Utc>,
        _: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolDiscoveredTvl>> {
        self.instants
            .lock()
            .unwrap()
            .push(("discoveries", last, protocol));
        Ok(vec![])
    }
}

/// A `SwapEventLookup` answering an empty page, which records the start of each window read.
#[derive(Default)]
pub(crate) struct MockSwapEventRepo {
    pub(crate) windows_from: Mutex<Vec<DateTime<Utc>>>,
}

#[async_trait]
impl SwapEventLookup for MockSwapEventRepo {
    async fn list_events(
        &self,
        _: EventScope,
        from: DateTime<Utc>,
        _: Option<EventCursor>,
        _: i64,
    ) -> RepositoryResult<Page<SwapEvent>> {
        self.windows_from.lock().unwrap().push(from);
        Ok(Page::empty())
    }
}

/// A `LiquidityChangeEventLookup` answering an empty page, which records the start of each
/// window read.
#[derive(Default)]
pub(crate) struct MockLiquidityChangeEventRepo {
    pub(crate) windows_from: Mutex<Vec<DateTime<Utc>>>,
}

#[async_trait]
impl LiquidityChangeEventLookup for MockLiquidityChangeEventRepo {
    async fn list_events(
        &self,
        _: EventScope,
        from: DateTime<Utc>,
        _: Option<EventCursor>,
        _: i64,
    ) -> RepositoryResult<Page<LiquidityChangeEvent>> {
        self.windows_from.lock().unwrap().push(from);
        Ok(Page::empty())
    }
}

/// A `PositionChangeEventLookup` answering an empty page, which records the start of each
/// window read.
#[derive(Default)]
pub(crate) struct MockPositionChangeEventRepo {
    pub(crate) windows_from: Mutex<Vec<DateTime<Utc>>>,
}

#[async_trait]
impl PositionChangeEventLookup for MockPositionChangeEventRepo {
    async fn list_events(
        &self,
        _: EventScope,
        from: DateTime<Utc>,
        _: Option<EventCursor>,
        _: i64,
    ) -> RepositoryResult<Page<PositionChangeEvent>> {
        self.windows_from.lock().unwrap().push(from);
        Ok(Page::empty())
    }
}

/// A `RewardChangeEventLookup` answering an empty page, which records the start of each
/// window read.
#[derive(Default)]
pub(crate) struct MockRewardChangeEventRepo {
    pub(crate) windows_from: Mutex<Vec<DateTime<Utc>>>,
}

#[async_trait]
impl RewardChangeEventLookup for MockRewardChangeEventRepo {
    async fn list_events(
        &self,
        _: EventScope,
        from: DateTime<Utc>,
        _: Option<EventCursor>,
        _: i64,
    ) -> RepositoryResult<Page<RewardChangeEvent>> {
        self.windows_from.lock().unwrap().push(from);
        Ok(Page::empty())
    }
}

/// A `PoolChangeEventLookup` answering an empty page, which records the start of each
/// window read.
#[derive(Default)]
pub(crate) struct MockPoolChangeEventRepo {
    pub(crate) windows_from: Mutex<Vec<DateTime<Utc>>>,
}

#[async_trait]
impl PoolChangeEventLookup for MockPoolChangeEventRepo {
    async fn list_events(
        &self,
        _: EventScope,
        from: DateTime<Utc>,
        _: Option<EventCursor>,
        _: i64,
    ) -> RepositoryResult<Page<PoolChangeEvent>> {
        self.windows_from.lock().unwrap().push(from);
        Ok(Page::empty())
    }
}

/// A `PositionFeeClaimEventLookup` answering an empty page, which records the start of each
/// window read.
#[derive(Default)]
pub(crate) struct MockPositionFeeClaimEventRepo {
    pub(crate) windows_from: Mutex<Vec<DateTime<Utc>>>,
}

#[async_trait]
impl PositionFeeClaimEventLookup for MockPositionFeeClaimEventRepo {
    async fn list_events(
        &self,
        _: EventScope,
        from: DateTime<Utc>,
        _: Option<EventCursor>,
        _: i64,
    ) -> RepositoryResult<Page<PositionFeeClaimEvent>> {
        self.windows_from.lock().unwrap().push(from);
        Ok(Page::empty())
    }
}

/// A `ProtocolFeeClaimEventLookup` answering an empty page, which records the start of each
/// window read.
#[derive(Default)]
pub(crate) struct MockProtocolFeeClaimEventRepo {
    pub(crate) windows_from: Mutex<Vec<DateTime<Utc>>>,
}

#[async_trait]
impl ProtocolFeeClaimEventLookup for MockProtocolFeeClaimEventRepo {
    async fn list_events(
        &self,
        _: EventScope,
        from: DateTime<Utc>,
        _: Option<EventCursor>,
        _: i64,
    ) -> RepositoryResult<Page<ProtocolFeeClaimEvent>> {
        self.windows_from.lock().unwrap().push(from);
        Ok(Page::empty())
    }
}

/// A `RewardClaimEventLookup` answering an empty page, which records the start of each
/// window read.
#[derive(Default)]
pub(crate) struct MockRewardClaimEventRepo {
    pub(crate) windows_from: Mutex<Vec<DateTime<Utc>>>,
}

#[async_trait]
impl RewardClaimEventLookup for MockRewardClaimEventRepo {
    async fn list_events(
        &self,
        _: EventScope,
        from: DateTime<Utc>,
        _: Option<EventCursor>,
        _: i64,
    ) -> RepositoryResult<Page<RewardClaimEvent>> {
        self.windows_from.lock().unwrap().push(from);
        Ok(Page::empty())
    }
}

/// A `PoolCreationEventLookup` answering an empty page, which records the start of each
/// window read.
#[derive(Default)]
pub(crate) struct MockPoolCreationEventRepo {
    pub(crate) windows_from: Mutex<Vec<DateTime<Utc>>>,
}

#[async_trait]
impl PoolCreationEventLookup for MockPoolCreationEventRepo {
    async fn list_events(
        &self,
        _: Option<Protocol>,
        from: DateTime<Utc>,
        _: Option<EventCursor>,
        _: i64,
    ) -> RepositoryResult<Page<PoolCreationEvent>> {
        self.windows_from.lock().unwrap().push(from);
        Ok(Page::empty())
    }
}

/// A `RotationRunLookup` set by each test: when the tranches were written through, when the
/// rotation was measured since, the scope and end each list was read with, and the hour each
/// series of bars is split from.
#[derive(Default)]
pub(crate) struct MockRotationRunRepo {
    pub(crate) widths: Mutex<Vec<TimeDelta>>,
    pub(crate) written_through: Option<DateTime<Utc>>,
    pub(crate) measured_since: Option<DateTime<Utc>>,
    pub(crate) lists: Mutex<Vec<(RotationRunScope, TranchePeriod)>>,
    pub(crate) splits_since: Mutex<Vec<Option<DateTime<Utc>>>>,
    pub(crate) tranche_splits: Mutex<Vec<(TranchePeriod, Option<DateTime<Utc>>)>>,
}

#[async_trait]
impl RotationRunLookup for MockRotationRunRepo {
    async fn find_measured_since(&self, _: Protocol) -> RepositoryResult<Option<DateTime<Utc>>> {
        Ok(self.measured_since)
    }

    async fn find_written_through(&self) -> RepositoryResult<Option<DateTime<Utc>>> {
        Ok(self.written_through)
    }

    async fn find_totals(
        &self,
        _: &TranchePeriod,
        _: Protocol,
    ) -> RepositoryResult<RotationRunTotals> {
        Ok(RotationRunTotals {
            runs: 0,
            pools: 0,
            in_progress: 0,
            median_duration_seconds: None,
            p90_duration_seconds: None,
            volume_usd: rust_decimal::Decimal::ZERO,
            first_hour: None,
        })
    }

    async fn list_runs(
        &self,
        period: &TranchePeriod,
        _: Window,
        scope: RotationRunScope,
        _: RotationRunOrder,
        _: Option<RotationRunCursor>,
        _: i64,
    ) -> RepositoryResult<Page<RotationRun>> {
        self.lists.lock().unwrap().push((scope, *period));
        Ok(Page::empty())
    }

    async fn list_hourly(
        &self,
        _: &FrozenPeriod,
        runs_since: Option<DateTime<Utc>>,
        _: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<RunSplit>>> {
        self.splits_since.lock().unwrap().push(runs_since);
        Ok(Vec::new())
    }

    async fn list_buckets(
        &self,
        _: &FrozenPeriod,
        width: TimeDelta,
        runs_since: Option<DateTime<Utc>>,
        _: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<RunSplit>>> {
        self.widths.lock().unwrap().push(width);
        self.splits_since.lock().unwrap().push(runs_since);
        Ok(Vec::new())
    }
    async fn list_tranches(
        &self,
        period: &TranchePeriod,
        runs_since: Option<DateTime<Utc>>,
        _: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<RunSplit>>> {
        self.tranche_splits
            .lock()
            .unwrap()
            .push((*period, runs_since));
        Ok(Vec::new())
    }
}
