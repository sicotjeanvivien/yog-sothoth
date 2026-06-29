//! Unit tests for `LiquidityService`.

use std::sync::Arc;

use yog_core::{PageDirection, PagePosition, RepositoryError};

use super::super::{MeteoraDammV2LiquidityListParams, MeteoraDammV2LiquidityService};
use crate::testing::{MockLiquidityEventRepo, make_liquidity_event, make_liquidity_page, pk};

fn default_params(pool: solana_pubkey::Pubkey) -> MeteoraDammV2LiquidityListParams {
    MeteoraDammV2LiquidityListParams {
        pool_address: pool,
        cursor: None,
        direction: PageDirection::Next,
        position: None,
        limit: 50,
    }
}

#[tokio::test]
async fn returns_page_items_from_repo() {
    let addr = pk(1);
    let event = make_liquidity_event(addr);
    let page = make_liquidity_page(vec![event.clone()], true, true);

    let svc = MeteoraDammV2LiquidityService::new(Arc::new(MockLiquidityEventRepo::with_page(page)));

    let result = svc
        .list_liquidity_for_pool(default_params(addr))
        .await
        .unwrap();

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].event.signature, event.signature);
}

#[tokio::test]
async fn empty_page_is_not_an_error() {
    let addr = pk(1);
    let svc = MeteoraDammV2LiquidityService::new(Arc::new(MockLiquidityEventRepo::empty()));

    let result = svc
        .list_liquidity_for_pool(default_params(addr))
        .await
        .unwrap();

    assert!(result.items.is_empty());
    assert!(result.is_first && result.is_last);
}

#[tokio::test]
async fn pagination_metadata_is_preserved() {
    let addr = pk(1);
    let event = make_liquidity_event(addr);
    let page = make_liquidity_page(vec![event], false, false);

    let svc = MeteoraDammV2LiquidityService::new(Arc::new(MockLiquidityEventRepo::with_page(page)));

    let result = svc
        .list_liquidity_for_pool(default_params(addr))
        .await
        .unwrap();

    assert!(!result.is_first);
    assert!(!result.is_last);
    assert!(result.prev_cursor.is_some());
    assert!(result.next_cursor.is_some());
}

#[tokio::test]
async fn last_page_has_no_next_cursor() {
    let addr = pk(1);
    let event = make_liquidity_event(addr);
    let page = make_liquidity_page(vec![event], false, true);

    let svc = MeteoraDammV2LiquidityService::new(Arc::new(MockLiquidityEventRepo::with_page(page)));

    let result = svc
        .list_liquidity_for_pool(default_params(addr))
        .await
        .unwrap();

    assert!(result.is_last);
    assert!(result.next_cursor.is_none());
    assert!(result.prev_cursor.is_some());
}

#[tokio::test]
async fn position_param_is_threaded_to_repo() {
    let addr = pk(1);
    let svc = MeteoraDammV2LiquidityService::new(Arc::new(MockLiquidityEventRepo::empty()));

    let params = MeteoraDammV2LiquidityListParams {
        pool_address: addr,
        cursor: None,
        direction: PageDirection::Prev,
        position: Some(PagePosition::Last),
        limit: 10,
    };

    svc.list_liquidity_for_pool(params).await.unwrap();
}

#[tokio::test]
async fn repo_error_propagates() {
    let addr = pk(1);
    let svc = MeteoraDammV2LiquidityService::new(Arc::new(MockLiquidityEventRepo::failing()));

    let err = svc
        .list_liquidity_for_pool(default_params(addr))
        .await
        .unwrap_err();

    assert!(matches!(err, RepositoryError::Integrity(_)));
}
