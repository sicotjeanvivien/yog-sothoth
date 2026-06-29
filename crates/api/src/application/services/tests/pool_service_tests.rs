//! Unit tests for `PoolService`. Mocks and fixtures come from
//! `crate::testing`; this file holds only the scenarios.

use super::PoolListParams;
use super::PoolService;
use crate::testing::make_pool_current_state;
use crate::testing::{
    MockAnalyticsRepo, MockMetadataRepo, MockPoolCurrentStateRepo, MockPriceRepo, PoolRepoOnce,
    make_metadata, make_page, make_pool, make_price, pk,
};
use std::sync::Arc;

use std::collections::HashMap;
use yog_core::domain::{PoolAnalytics, PoolRankMetric};
use yog_core::{PageDirection, PoolSort};

fn service(
    pool_repo: PoolRepoOnce,
    pool_current_state_repo: MockPoolCurrentStateRepo,
    analytics: MockAnalyticsRepo,
    metadata: MockMetadataRepo,
    price: MockPriceRepo,
) -> PoolService {
    PoolService::new(
        Arc::new(pool_repo),
        Arc::new(pool_current_state_repo),
        Arc::new(analytics),
        Arc::new(metadata),
        Arc::new(price),
    )
}

fn default_params() -> PoolListParams {
    PoolListParams {
        cursor: None,
        direction: PageDirection::Next,
        position: None,
        sort: PoolSort::FirstSeenAsc,
        search: None,
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

    assert!(svc.get_pool(&pk(99)).await.unwrap().is_none());
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

    let enriched = svc.get_pool(&addr).await.unwrap().unwrap();
    assert_eq!(enriched.pool.pool_address, addr);
    assert_eq!(
        enriched.token_a.metadata.as_ref().unwrap().symbol,
        Some("AAA".to_string())
    );
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

    let enriched = svc.get_pool(&addr).await.unwrap().unwrap();
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
