//! Test support: fixtures and repository mocks shared by every service test. The mocks
//! implement only the read lenses the services consume; a "once" mock yields its value a
//! single time, and a second call panics.

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
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
        Announcement, AnnouncementKind, AnnouncementLookup, AnnouncementSeverity, DiscoveredTvl,
        EventFreshnessRepository, FrozenHours, LatestProtocolTvl, NetworkStatus,
        NetworkStatusLookup, Pool, PoolCatalog, Protocol, ProtocolActivity, ProtocolActivityBar,
        ProtocolActivityLookup, ProtocolPools, ProtocolTvl, Severity, Signal, SignalCursor,
        SignalFeed, SignalRecord, Span, TokenActivityCursor, TokenActivityFeed, TokenActivityPage,
        TokenMetadata, TokenMetadataLookup, TokenPrice, TokenPriceLookup, Window,
    },
};

use crate::application::{
    ActivityService, AnnouncementService, IngestionService, NetworkService, ProtocolCatalogService,
    SignalService, TokenService, WorkSlots,
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
        activity_service: Arc::new(ActivityService::new(
            Arc::new(MockProtocolActivityRepo::new(None)),
            Arc::new(MockTokenActivityRepo::default()),
        )),
        work_slots,
        token_service: Arc::new(TokenService::new(
            Arc::new(MockMetadataRepo::empty()),
            Arc::new(MockPriceRepo::empty()),
        )),
        announcement_service: Arc::new(AnnouncementService::new(Arc::new(
            MockAnnouncementRepo::failing(),
        ))),
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
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let response = build_router(app_state(), vec![])
        .oneshot(request)
        .await
        .unwrap();
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
}

/// A `ProtocolActivityLookup` that answers `frozen` and the seeded rows every time, and
/// records the span of each read.
pub(crate) struct MockProtocolActivityRepo {
    frozen: Option<FrozenHours>,
    pub(crate) activity: Vec<ProtocolActivity>,
    pub(crate) bars: Vec<ProtocolActivityBar>,
    pub(crate) tvl: Vec<ProtocolTvl>,
    /// The discovered parts `tvl_latest` hands with each of [`Self::tvl`].
    pub(crate) discovered: Vec<DiscoveredTvl>,
    pub(crate) spans: Mutex<Vec<Span>>,
    pub(crate) tvl_ranges: Mutex<Vec<(DateTime<Utc>, DateTime<Utc>)>>,
}

impl MockProtocolActivityRepo {
    /// The hours `tvl_latest` was asked about, as `(hour, hour)` in [`Self::tvl_ranges`].
    fn record_tvl(&self, first: DateTime<Utc>, last: DateTime<Utc>) -> Vec<ProtocolTvl> {
        self.tvl_ranges.lock().unwrap().push((first, last));
        self.tvl.clone()
    }
}

impl MockProtocolActivityRepo {
    pub(crate) fn new(frozen: Option<FrozenHours>) -> Self {
        Self {
            frozen,
            activity: vec![],
            bars: vec![],
            tvl: vec![],
            discovered: vec![],
            spans: Mutex::new(vec![]),
            tvl_ranges: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl ProtocolActivityLookup for MockProtocolActivityRepo {
    async fn frozen_hours(&self) -> RepositoryResult<Option<FrozenHours>> {
        Ok(self.frozen)
    }
    async fn activity(&self, span: &Span, _: Window) -> RepositoryResult<Vec<ProtocolActivity>> {
        self.spans.lock().unwrap().push(*span);
        Ok(self.activity.clone())
    }
    async fn activity_hourly(&self, span: &Span) -> RepositoryResult<Vec<ProtocolActivityBar>> {
        self.spans.lock().unwrap().push(*span);
        Ok(self.bars.clone())
    }
    async fn activity_daily(&self, span: &Span) -> RepositoryResult<Vec<ProtocolActivityBar>> {
        self.spans.lock().unwrap().push(*span);
        Ok(self.bars.clone())
    }
    async fn tvl_latest(&self, last: DateTime<Utc>) -> RepositoryResult<Vec<LatestProtocolTvl>> {
        Ok(self
            .record_tvl(last, last)
            .into_iter()
            .map(|tvl| LatestProtocolTvl {
                tvl,
                discovered: self.discovered.clone(),
            })
            .collect())
    }
    async fn tvl_hourly(
        &self,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
    ) -> RepositoryResult<Vec<ProtocolTvl>> {
        Ok(self.record_tvl(first, last))
    }
}

/// A `TokenActivityFeed` whose last ranking was written at `ranked_at`, `None`
/// for none: it records each window read and answers an empty page of it.
#[derive(Default)]
pub(crate) struct MockTokenActivityRepo {
    pub(crate) ranked_at: Option<DateTime<Utc>>,
    pub(crate) reads: Mutex<Vec<Window>>,
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
}

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

pub(crate) struct MockAnnouncementRepo {
    list_active: Mutex<Option<RepositoryResult<Vec<Announcement>>>>,
}

impl MockAnnouncementRepo {
    pub(crate) fn active(announcements: Vec<Announcement>) -> Self {
        Self {
            list_active: Mutex::new(Some(Ok(announcements))),
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            list_active: Mutex::new(Some(Err(RepositoryError::Integrity(
                "announcement boom".into(),
            )))),
        }
    }
}

#[async_trait]
impl AnnouncementLookup for MockAnnouncementRepo {
    async fn list_active(&self, _now: DateTime<Utc>) -> RepositoryResult<Vec<Announcement>> {
        take(&self.list_active)
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

pub(crate) fn make_announcement(id: i64) -> Announcement {
    Announcement {
        id,
        kind: AnnouncementKind::Release,
        severity: AnnouncementSeverity::Info,
        message: format!("announcement {id}"),
        link_url: None,
        starts_at: ts(1_000),
        ends_at: None,
    }
}

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
        _pool: Option<Pubkey>,
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

/// Slots for tests that do not exercise them: plenty, and a short wait.
pub(crate) fn work_slots() -> crate::application::WorkSlots {
    crate::application::WorkSlots::new(8, std::time::Duration::from_millis(100))
}
