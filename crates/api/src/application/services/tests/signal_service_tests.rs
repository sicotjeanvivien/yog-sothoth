//! Unit tests for `SignalService`.

use std::sync::Arc;

use yog_core::{PageDirection, RepositoryError};

use super::super::{SignalListParams, SignalService};
use crate::testing::{
    MockMetadataRepo, MockSignalRepo, PoolRepoOnce, make_metadata, make_pool, make_signal_page,
    make_signal_record, pk,
};

fn default_params() -> SignalListParams {
    SignalListParams {
        severity: None,
        protocol: None,
        pool: None,
        cursor: None,
        direction: PageDirection::Next,
        position: None,
        limit: 50,
    }
}

/// A service over a feed page, a pool catalog and metadata; no price.
fn service(
    signals: MockSignalRepo,
    pools: PoolRepoOnce,
    metadata: MockMetadataRepo,
) -> SignalService {
    SignalService::new(Arc::new(signals), Arc::new(pools), Arc::new(metadata))
}

#[tokio::test]
async fn returns_page_items_enriched_with_token_pair() {
    let record = make_signal_record(42, pk(1));
    let page = make_signal_page(vec![record.clone()], true);

    let svc = service(
        MockSignalRepo::with_page(page),
        PoolRepoOnce::with_pools(vec![make_pool(pk(1), pk(2), pk(3))]),
        MockMetadataRepo::with(vec![
            (pk(2), make_metadata(pk(2), "SOL")),
            (pk(3), make_metadata(pk(3), "USDC")),
        ]),
    );

    let result = svc.list_signals(default_params()).await.unwrap();

    assert_eq!(result.items.len(), 1);
    let item = &result.items[0];
    assert_eq!(item.record.id, 42);
    assert_eq!(item.record.signal.pool_address, pk(1));
    assert_eq!(
        item.token_a.metadata.as_ref().unwrap().symbol.as_deref(),
        Some("SOL")
    );
    assert_eq!(
        item.token_b.metadata.as_ref().unwrap().symbol.as_deref(),
        Some("USDC")
    );
}

#[tokio::test]
async fn empty_page_is_not_an_error() {
    let svc = service(
        MockSignalRepo::empty(),
        PoolRepoOnce::with_pools(vec![]),
        MockMetadataRepo::empty(),
    );

    let result = svc.list_signals(default_params()).await.unwrap();

    assert!(result.items.is_empty());
    assert!(result.is_first);
    assert!(result.is_last);
}

#[tokio::test]
async fn pool_unknown_to_catalog_yields_unresolved_sides() {
    let record = make_signal_record(7, pk(9));
    let page = make_signal_page(vec![record], true);

    // The catalog omits the address: the signal still ships.
    let svc = service(
        MockSignalRepo::with_page(page),
        PoolRepoOnce::with_pools(vec![]),
        MockMetadataRepo::empty(),
    );

    let result = svc.list_signals(default_params()).await.unwrap();

    let item = &result.items[0];
    assert!(item.token_a.mint.is_none());
    assert!(item.token_a.metadata.is_none());
    assert!(item.token_b.mint.is_none());
}

#[tokio::test]
async fn missing_metadata_keeps_mint_without_symbol() {
    let record = make_signal_record(8, pk(1));
    let page = make_signal_page(vec![record], true);

    // Mints resolved, but yog-context hasn't fetched metadata yet.
    let svc = service(
        MockSignalRepo::with_page(page),
        PoolRepoOnce::with_pools(vec![make_pool(pk(1), pk(2), pk(3))]),
        MockMetadataRepo::empty(),
    );

    let result = svc.list_signals(default_params()).await.unwrap();

    let item = &result.items[0];
    assert_eq!(item.token_a.mint, Some(pk(2)));
    assert!(item.token_a.metadata.is_none());
}

/// Mutation: a lookup per signal — `PoolRepoOnce` panics on the second.
#[tokio::test]
async fn enrich_batch_resolves_every_pair_through_one_lookup() {
    let records = vec![make_signal_record(21, pk(1)), make_signal_record(22, pk(1))];

    let svc = service(
        MockSignalRepo::empty(),
        PoolRepoOnce::with_pools(vec![make_pool(pk(1), pk(2), pk(3))]),
        MockMetadataRepo::with(vec![(pk(2), make_metadata(pk(2), "SOL"))]),
    );

    let enriched = svc.enrich_batch(records).await.unwrap();

    assert_eq!(enriched.len(), 2);
    for signal in &enriched {
        assert_eq!(
            signal.token_a.metadata.as_ref().unwrap().symbol.as_deref(),
            Some("SOL")
        );
        // Mint B resolved, metadata not yet fetched.
        assert_eq!(signal.token_b.mint, Some(pk(3)));
        assert!(signal.token_b.metadata.is_none());
    }
}

#[tokio::test]
async fn repository_error_bubbles_up() {
    let svc = service(
        MockSignalRepo::failing(),
        PoolRepoOnce::with_pools(vec![]),
        MockMetadataRepo::empty(),
    );

    let err = svc.list_signals(default_params()).await.unwrap_err();

    assert!(matches!(err, RepositoryError::Integrity(_)));
}

#[tokio::test]
async fn enrichment_error_bubbles_up() {
    let record = make_signal_record(42, pk(1));
    let page = make_signal_page(vec![record], true);

    let svc = service(
        MockSignalRepo::with_page(page),
        PoolRepoOnce::with_pools(vec![make_pool(pk(1), pk(2), pk(3))]),
        MockMetadataRepo::failing(),
    );

    let err = svc.list_signals(default_params()).await.unwrap_err();

    assert!(matches!(err, RepositoryError::Integrity(_)));
}
