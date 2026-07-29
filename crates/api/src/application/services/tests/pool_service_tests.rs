//! Unit tests for `PoolService`. Mocks and fixtures come from
//! `crate::testing`; this file holds only the scenarios.

use super::PoolService;
use crate::testing::make_pool_current_state;
use crate::testing::{
    MockAnalyticsRepo, MockMetadataRepo, MockPoolCurrentStateRepo, MockPriceRepo, MockSignalRepo,
    PoolRepoOnce, make_metadata, make_page, make_pool, make_price, make_signal_record, pk,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use std::collections::HashMap;
use yog_core::domain::{
    MeteoraDammV2PoolProperties, Pool, PoolAnalytics, PoolListQuery, PoolProperties,
    PoolPropertiesLookup, PoolRankMetric, Protocol,
};
use yog_core::{PageDirection, PoolSort};

fn service(
    pool_repo: PoolRepoOnce,
    pool_current_state_repo: MockPoolCurrentStateRepo,
    analytics: MockAnalyticsRepo,
    metadata: MockMetadataRepo,
    price: MockPriceRepo,
) -> PoolService {
    // Most scenarios don't care about the signal indicator: a quiet
    // window (empty map) is the neutral default.
    service_with_signals(
        pool_repo,
        pool_current_state_repo,
        analytics,
        metadata,
        price,
        MockSignalRepo::recent_empty(),
    )
}

fn service_with_signals(
    pool_repo: PoolRepoOnce,
    pool_current_state_repo: MockPoolCurrentStateRepo,
    analytics: MockAnalyticsRepo,
    metadata: MockMetadataRepo,
    price: MockPriceRepo,
    signals: MockSignalRepo,
) -> PoolService {
    PoolService::new(
        Arc::new(pool_repo),
        Arc::new(pool_current_state_repo),
        Arc::new(analytics),
        Arc::new(metadata),
        Arc::new(price),
        Arc::new(signals),
        vec![Arc::new(MockPropertiesLookup::empty(
            Protocol::MeteoraDammV2,
        ))],
    )
}

/// A `PoolPropertiesLookup` for one protocol, recording whether it was consulted.
///
/// The satellite is not what most of these tests exercise — they cover the
/// enrichment pipeline (tokens, analytics, signals) — so the default answers
/// `None`, which is the realistic case anyway: most pools have no row until
/// yog-context or a genesis event fills one. The call counter exists for the
/// routing tests, which assert on *who was asked*, not just on what came back.
struct MockPropertiesLookup {
    protocol: Protocol,
    answer: Option<PoolProperties>,
    calls: AtomicUsize,
}

impl MockPropertiesLookup {
    fn empty(protocol: Protocol) -> Self {
        Self {
            protocol,
            answer: None,
            calls: AtomicUsize::new(0),
        }
    }

    fn with(protocol: Protocol, answer: PoolProperties) -> Self {
        Self {
            protocol,
            answer: Some(answer),
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl PoolPropertiesLookup for MockPropertiesLookup {
    fn protocol(&self) -> Protocol {
        self.protocol
    }

    async fn find_by_pool(
        &self,
        _: &solana_pubkey::Pubkey,
    ) -> yog_core::RepositoryResult<Option<PoolProperties>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.answer.clone())
    }
}

fn damm_v2_properties(addr: solana_pubkey::Pubkey) -> PoolProperties {
    PoolProperties::MeteoraDammV2(MeteoraDammV2PoolProperties {
        pool_address: addr,
        protocol_fee_percent: Some(20),
        referral_fee_percent: None,
        base_fee_kind: Some("constant".to_string()),
        has_dynamic_fee: Some(false),
    })
}

fn default_params() -> PoolListQuery {
    PoolListQuery {
        cursor: None,
        direction: PageDirection::Next,
        position: None,
        sort: PoolSort::FirstSeenAsc,
        search: None,
        fee_bps: None,
        limit: 50,
    }
}

#[tokio::test]
async fn missing_analytics_falls_back_to_empty() {
    let addr = pk(1);
    let pool = make_pool(addr, pk(10), pk(11));

    let svc = service(
        PoolRepoOnce::with_page(make_page(vec![pool], true, true)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    let page = svc.list_pools(default_params()).await.unwrap();

    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].analytics.tvl_usd.is_none());
    assert!(page.items[0].analytics.volume_24h_usd.is_none());
}

#[tokio::test]
async fn pagination_metadata_is_preserved() {
    let pool = make_pool(pk(1), pk(10), pk(11));
    let svc = service(
        PoolRepoOnce::with_page(make_page(vec![pool], false, false)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    let page = svc.list_pools(default_params()).await.unwrap();

    assert!(!page.is_first);
    assert!(!page.is_last);
    assert!(page.prev_cursor.is_some());
    assert!(page.next_cursor.is_some());
}

#[tokio::test]
async fn single_page_reports_both_boundaries() {
    let pool = make_pool(pk(1), pk(10), pk(11));
    let svc = service(
        PoolRepoOnce::with_page(make_page(vec![pool], true, true)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    let page = svc.list_pools(default_params()).await.unwrap();

    assert!(page.is_first);
    assert!(page.is_last);
    assert!(page.prev_cursor.is_none());
    assert!(page.next_cursor.is_none());
}

#[tokio::test]
async fn enrichment_tolerates_missing_metadata_and_price() {
    let pool = make_pool(pk(1), pk(10), pk(11));
    let svc = service(
        PoolRepoOnce::with_page(make_page(vec![pool], true, true)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    let page = svc.list_pools(default_params()).await.unwrap();
    let item = &page.items[0];

    assert!(item.token_a.metadata.is_none());
    assert!(item.token_a.price.is_none());
    assert!(item.token_b.metadata.is_none());
    assert!(item.token_b.price.is_none());
}

#[tokio::test]
async fn token_sides_map_to_their_own_mint() {
    let token_a = pk(10);
    let token_b = pk(11);
    let pool = make_pool(pk(1), token_a, token_b);

    let svc = service(
        PoolRepoOnce::with_page(make_page(vec![pool], true, true)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::with(vec![
            (token_a, make_metadata(token_a, "AAA")),
            (token_b, make_metadata(token_b, "BBB")),
        ]),
        MockPriceRepo::empty(),
    );

    let page = svc.list_pools(default_params()).await.unwrap();
    let item = &page.items[0];

    assert_eq!(item.token_a.mint, Some(token_a));
    assert_eq!(item.token_b.mint, Some(token_b));
    assert_eq!(
        item.token_a.metadata.as_ref().unwrap().symbol,
        Some("AAA".to_string())
    );
    assert_eq!(
        item.token_b.metadata.as_ref().unwrap().symbol,
        Some("BBB".to_string())
    );
}

#[tokio::test]
async fn partial_enrichment_one_side_only() {
    let token_a = pk(10);
    let token_b = pk(11);
    let pool = make_pool(pk(1), token_a, token_b);

    let svc = service(
        PoolRepoOnce::with_page(make_page(vec![pool], true, true)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::with(vec![(token_a, make_metadata(token_a, "AAA"))]),
        MockPriceRepo::with(vec![(token_a, make_price(token_a))]),
    );

    let page = svc.list_pools(default_params()).await.unwrap();
    let item = &page.items[0];

    assert!(item.token_a.metadata.is_some());
    assert!(item.token_a.price.is_some());
    assert!(item.token_b.metadata.is_none());
    assert!(item.token_b.price.is_none());
}

#[tokio::test]
async fn get_pool_returns_none_for_unknown_pool() {
    let svc = service(
        PoolRepoOnce::with_pool(None),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    assert!(svc.get_pool_detail(&pk(99)).await.unwrap().is_none());
}

#[tokio::test]
async fn get_pool_enriches_found_pool() {
    let addr = pk(1);
    let token_a = pk(10);
    let pool = make_pool(addr, token_a, pk(11));

    let svc = service(
        PoolRepoOnce::with_pool(Some(pool)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::with(vec![(token_a, make_metadata(token_a, "AAA"))]),
        MockPriceRepo::empty(),
    );

    let detail = svc.get_pool_detail(&addr).await.unwrap().unwrap();
    let enriched = detail.pool;
    assert_eq!(enriched.pool.pool_address, addr);
    assert_eq!(
        enriched.token_a.metadata.as_ref().unwrap().symbol,
        Some("AAA".to_string())
    );
}

/// The pool's own protocol picks the lookup, and its answer reaches the detail
/// sheet as-is. Registering the lookup is the whole wiring — the service names
/// no protocol.
#[tokio::test]
async fn get_pool_detail_routes_to_the_matching_protocol_lookup() {
    let addr = pk(1);
    let lookup = Arc::new(MockPropertiesLookup::with(
        Protocol::MeteoraDammV2,
        damm_v2_properties(addr),
    ));

    let svc = PoolService::new(
        Arc::new(PoolRepoOnce::with_pool(Some(make_pool(
            addr,
            pk(10),
            pk(11),
        )))),
        Arc::new(MockPoolCurrentStateRepo::not_found()),
        Arc::new(MockAnalyticsRepo::empty()),
        Arc::new(MockMetadataRepo::empty()),
        Arc::new(MockPriceRepo::empty()),
        Arc::new(MockSignalRepo::recent_empty()),
        vec![lookup.clone()],
    );

    let detail = svc.get_pool_detail(&addr).await.unwrap().unwrap();

    assert_eq!(lookup.calls(), 1);
    assert_eq!(detail.properties, Some(damm_v2_properties(addr)));
}

/// A pool of a protocol with no registered lookup yields no properties **and no
/// round-trip**: the registered lookup is never consulted.
///
/// This is the regression guard for the coupling this module used to have. With
/// a hard-wired DAMM v2 repository, a DLMM pool either queried the cp-amm
/// satellite for nothing or relied on a `match` arm someone had to remember to
/// write; neither is possible now.
#[tokio::test]
async fn get_pool_detail_skips_a_protocol_with_no_lookup() {
    let addr = pk(2);
    let pool = Pool {
        protocol: Protocol::MeteoraDlmm,
        ..make_pool(addr, pk(10), pk(11))
    };
    let damm_v2_lookup = Arc::new(MockPropertiesLookup::with(
        Protocol::MeteoraDammV2,
        damm_v2_properties(addr),
    ));

    let svc = PoolService::new(
        Arc::new(PoolRepoOnce::with_pool(Some(pool))),
        Arc::new(MockPoolCurrentStateRepo::not_found()),
        Arc::new(MockAnalyticsRepo::empty()),
        Arc::new(MockMetadataRepo::empty()),
        Arc::new(MockPriceRepo::empty()),
        Arc::new(MockSignalRepo::recent_empty()),
        vec![damm_v2_lookup.clone()],
    );

    let detail = svc.get_pool_detail(&addr).await.unwrap().unwrap();

    assert_eq!(damm_v2_lookup.calls(), 0);
    assert_eq!(detail.properties, None);
}

#[tokio::test]
async fn paginate_error_propagates() {
    let svc = service(
        PoolRepoOnce::with_paginate_err(),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    assert!(svc.list_pools(default_params()).await.is_err());
}

#[tokio::test]
async fn analytics_error_propagates() {
    let pool = make_pool(pk(1), pk(10), pk(11));
    let svc = service(
        PoolRepoOnce::with_page(make_page(vec![pool], true, true)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::failing(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    assert!(svc.list_pools(default_params()).await.is_err());
}

#[tokio::test]
async fn present_analytics_are_attached_to_the_right_pool() {
    let addr = pk(1);
    let pool = make_pool(addr, pk(10), pk(11));

    let mut map = std::collections::HashMap::new();
    map.insert(
        addr,
        PoolAnalytics {
            tvl_usd: Some(rust_decimal::Decimal::new(1000, 0)),
            volume_24h_usd: Some(rust_decimal::Decimal::new(500, 0)),
            ..PoolAnalytics::empty()
        },
    );

    let svc = service(
        PoolRepoOnce::with_page(make_page(vec![pool], true, true)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::with(map),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    let page = svc.list_pools(default_params()).await.unwrap();
    let analytics = &page.items[0].analytics;
    assert_eq!(analytics.tvl_usd, Some(rust_decimal::Decimal::new(1000, 0)));
    assert_eq!(
        analytics.volume_24h_usd,
        Some(rust_decimal::Decimal::new(500, 0))
    );
}

#[tokio::test]
async fn recent_signals_are_attached_to_the_right_pool() {
    let with_signals = pk(1);
    let quiet = pk(2);
    let pools = vec![
        make_pool(with_signals, pk(10), pk(11)),
        make_pool(quiet, pk(12), pk(13)),
    ];

    let mut map = HashMap::new();
    map.insert(
        with_signals,
        vec![
            make_signal_record(2, with_signals),
            make_signal_record(1, with_signals),
        ],
    );

    let svc = service_with_signals(
        PoolRepoOnce::with_page(make_page(pools, true, true)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
        MockSignalRepo::with_recent(map),
    );

    let page = svc.list_pools(default_params()).await.unwrap();

    assert_eq!(page.items[0].recent_signals.len(), 2);
    assert_eq!(page.items[0].recent_signals[0].id, 2);
    assert!(page.items[1].recent_signals.is_empty());
}

// ---------------------------------------------------------------------------
// get_latest_state
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_latest_state_returns_none_when_no_state() {
    // A pool may exist via Claim* events without ever appearing in the
    // current-state projection. The service must surface this as None,
    // not an error — the handler maps None to a 404 with a specific
    // message distinct from "pool not found".
    let svc = service(
        PoolRepoOnce::with_pool(None),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    let result = svc.get_latest_state("anyaddr").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn get_latest_state_returns_state_when_present() {
    let addr = pk(1);
    let state = make_pool_current_state(addr);

    let svc = service(
        PoolRepoOnce::with_pool(None),
        MockPoolCurrentStateRepo::found(state.clone()),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    let result = svc
        .get_latest_state(&addr.to_string())
        .await
        .unwrap()
        .expect("should be Some");

    assert_eq!(result.state.pool_address, state.pool_address);
    // The pool's mints/metadata are unresolved here (with_pool(None),
    // empty metadata), so the spot price cannot be rescaled → None,
    // never a fabricated value.
    assert!(result.spot_price_a_in_b.is_none());
}

#[tokio::test]
async fn get_latest_state_derives_spot_price_when_resolvable() {
    use rust_decimal::prelude::ToPrimitive;

    let addr = pk(1);
    let (mint_a, mint_b) = (pk(10), pk(11));
    let state = make_pool_current_state(addr); // last_sqrt_price = 1e18

    let svc = service(
        PoolRepoOnce::with_pool(Some(make_pool(addr, mint_a, mint_b))),
        MockPoolCurrentStateRepo::found(state),
        MockAnalyticsRepo::empty(),
        // Both sides resolved (make_metadata → 9 decimals), so the
        // sqrt_price can be decoded to a human spot price.
        MockMetadataRepo::with(vec![
            (mint_a, make_metadata(mint_a, "AAA")),
            (mint_b, make_metadata(mint_b, "BBB")),
        ]),
        MockPriceRepo::empty(),
    );

    let view = svc
        .get_latest_state(&addr.to_string())
        .await
        .unwrap()
        .expect("should be Some");

    // (1e18 / 2^64)^2 * 10^(9-9) ≈ 0.00293874 — the value the core helper
    // computes; the service plumbing must surface it intact.
    let price = view
        .spot_price_a_in_b
        .expect("spot price resolvable")
        .to_f64()
        .unwrap();
    assert!(
        (price - 0.002_938_74).abs() < 0.000_001,
        "got {price}, expected ~0.00293874"
    );
}

#[tokio::test]
async fn get_latest_state_propagates_repo_error() {
    let svc = service(
        PoolRepoOnce::with_pool(None),
        MockPoolCurrentStateRepo::failing(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    assert!(svc.get_latest_state("anyaddr").await.is_err());
}

#[tokio::test]
async fn get_pool_attaches_analytics_correctly() {
    let addr = pk(1);
    let pool = make_pool(addr, pk(10), pk(11));

    let mut map = std::collections::HashMap::new();
    map.insert(
        addr,
        PoolAnalytics {
            tvl_usd: Some(rust_decimal::Decimal::new(2000, 0)),
            volume_24h_usd: Some(rust_decimal::Decimal::new(750, 0)),
            ..PoolAnalytics::empty()
        },
    );

    let svc = service(
        PoolRepoOnce::with_pool(Some(pool)),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::with(map),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    let enriched = svc.get_pool_detail(&addr).await.unwrap().unwrap().pool;
    assert_eq!(
        enriched.analytics.tvl_usd,
        Some(rust_decimal::Decimal::new(2000, 0))
    );
}

// ---------------------------------------------------------------------------
// top_pools
// ---------------------------------------------------------------------------

#[tokio::test]
async fn top_pools_emits_in_rank_order() {
    // The ranking puts a1 first, a2 second. The batch `find_by_addresses`
    // returns them in the *opposite* order on purpose — the service must
    // re-impose the rank, not echo the DB's arbitrary order.
    let a1 = pk(1);
    let a2 = pk(2);
    let pool1 = make_pool(a1, pk(10), pk(11));
    let pool2 = make_pool(a2, pk(12), pk(13));

    let mut map = HashMap::new();
    map.insert(a1, PoolAnalytics::empty());
    map.insert(a2, PoolAnalytics::empty());

    let svc = service(
        PoolRepoOnce::with_pools(vec![pool2, pool1]),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::with(map).with_top(vec![a1, a2]),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    let top = svc.top_pools(PoolRankMetric::Volume24h, 10).await.unwrap();

    assert_eq!(top.len(), 2);
    assert_eq!(top[0].pool.pool_address, a1);
    assert_eq!(top[1].pool.pool_address, a2);
}

#[tokio::test]
async fn top_pools_empty_when_no_ranking() {
    // No ranked addresses → short-circuit to an empty list, no pool/analytics
    // reads attempted.
    let svc = service(
        PoolRepoOnce::with_pools(vec![]),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::empty(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    let top = svc.top_pools(PoolRankMetric::Volume24h, 10).await.unwrap();
    assert!(top.is_empty());
}

#[tokio::test]
async fn top_pools_ranking_error_propagates() {
    let svc = service(
        PoolRepoOnce::with_pools(vec![]),
        MockPoolCurrentStateRepo::not_found(),
        MockAnalyticsRepo::failing(),
        MockMetadataRepo::empty(),
        MockPriceRepo::empty(),
    );

    assert!(svc.top_pools(PoolRankMetric::Volume24h, 10).await.is_err());
}
