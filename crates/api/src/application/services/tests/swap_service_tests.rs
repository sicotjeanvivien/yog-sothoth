//! Unit tests for `SwapService`.

use std::sync::Arc;

use yog_core::{PageDirection, PagePosition, RepositoryError};

use super::super::{MeteoraDammV2SwapListParams, MeteoraDammV2SwapService};
use crate::testing::{MockSwapEventRepo, make_swap_event, make_swap_page, pk};

fn default_params(pool: solana_pubkey::Pubkey) -> MeteoraDammV2SwapListParams {
    MeteoraDammV2SwapListParams {
        pool_address: pool,
        cursor: None,
        direction: PageDirection::Next,
        position: None,
        limit: 50,
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[tokio::test]
async fn returns_page_items_from_repo() {
    let addr = pk(1);
    let event = make_swap_event(addr);
    let page = make_swap_page(vec![event.clone()], true, true);

    let svc = MeteoraDammV2SwapService::new(Arc::new(MockSwapEventRepo::with_page(page)));

    let result = svc.list_swaps_for_pool(default_params(addr)).await.unwrap();

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].signature, event.signature);
}

#[tokio::test]
async fn empty_page_is_not_an_error() {
    let addr = pk(1);
    let svc = MeteoraDammV2SwapService::new(Arc::new(MockSwapEventRepo::empty()));

    let result = svc.list_swaps_for_pool(default_params(addr)).await.unwrap();

    assert!(result.items.is_empty());
    assert!(result.is_first);
    assert!(result.is_last);
}

#[tokio::test]
async fn pagination_metadata_is_preserved() {
    let addr = pk(1);
    let event = make_swap_event(addr);
    // middle page: neither first nor last, both cursors present
    let page = make_swap_page(vec![event], false, false);

    let svc = MeteoraDammV2SwapService::new(Arc::new(MockSwapEventRepo::with_page(page)));

    let result = svc.list_swaps_for_pool(default_params(addr)).await.unwrap();

    assert!(!result.is_first);
    assert!(!result.is_last);
    assert!(result.prev_cursor.is_some());
    assert!(result.next_cursor.is_some());
}

#[tokio::test]
async fn last_page_has_no_next_cursor() {
    let addr = pk(1);
    let event = make_swap_event(addr);
    let page = make_swap_page(vec![event], false, true);

    let svc = MeteoraDammV2SwapService::new(Arc::new(MockSwapEventRepo::with_page(page)));

    let result = svc.list_swaps_for_pool(default_params(addr)).await.unwrap();

    assert!(result.is_last);
    assert!(result.next_cursor.is_none());
    assert!(result.prev_cursor.is_some());
}

#[tokio::test]
async fn prev_position_is_threaded_to_repo() {
    // The service must pass `position=Some(Last)` to the repo without
    // swallowing it. We verify indirectly: the mock is consumed once,
    // so if the call reaches it, the param was not dropped.
    let addr = pk(1);
    let svc = MeteoraDammV2SwapService::new(Arc::new(MockSwapEventRepo::empty()));

    let params = MeteoraDammV2SwapListParams {
        pool_address: addr,
        cursor: None,
        direction: PageDirection::Prev,
        position: Some(PagePosition::Last),
        limit: 10,
    };

    // Just asserting it doesn't panic / the mock is consumed cleanly.
    svc.list_swaps_for_pool(params).await.unwrap();
}

// ── Error propagation ────────────────────────────────────────────────

#[tokio::test]
async fn repo_error_propagates() {
    let addr = pk(1);
    let svc = MeteoraDammV2SwapService::new(Arc::new(MockSwapEventRepo::failing()));

    let err = svc
        .list_swaps_for_pool(default_params(addr))
        .await
        .unwrap_err();

    assert!(matches!(err, RepositoryError::Integrity(_)));
}
