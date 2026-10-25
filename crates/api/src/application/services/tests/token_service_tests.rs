//! Unit tests for `TokenService`.

use std::sync::Arc;

use yog_core::RepositoryError;

use super::super::TokenService;
use crate::testing::{MockMetadataRepo, MockPriceRepo, make_metadata, make_price, pk};

#[tokio::test]
async fn returns_aggregate_with_metadata_and_price() {
    let mint = pk(20);

    let svc = TokenService::new(
        Arc::new(MockMetadataRepo::with(vec![(
            mint,
            make_metadata(mint, "SOL"),
        )])),
        Arc::new(MockPriceRepo::with(vec![(mint, make_price(mint))])),
    );

    let agg = svc.get_token(&mint).await.unwrap().expect("should be Some");

    assert_eq!(agg.metadata.mint, mint);
    assert_eq!(agg.metadata.symbol, Some("SOL".to_string()));
    assert!(agg.price.is_some());
}

#[tokio::test]
async fn returns_aggregate_without_price() {
    // Metadata exists but the price worker has not run yet.
    let mint = pk(20);

    let svc = TokenService::new(
        Arc::new(MockMetadataRepo::with(vec![(
            mint,
            make_metadata(mint, "SOL"),
        )])),
        Arc::new(MockPriceRepo::empty()),
    );

    let agg = svc.get_token(&mint).await.unwrap().expect("should be Some");

    assert!(agg.price.is_none());
}

#[tokio::test]
async fn returns_none_for_unknown_mint() {
    let mint = pk(99);

    let svc = TokenService::new(
        Arc::new(MockMetadataRepo::empty()),
        Arc::new(MockPriceRepo::empty()),
    );

    let result = svc.get_token(&mint).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn correct_mint_is_returned_in_aggregate() {
    // Mutation: the aggregate carrying another mint of the repository.
    let mint_a = pk(10);
    let mint_b = pk(11);

    let svc = TokenService::new(
        Arc::new(MockMetadataRepo::with(vec![
            (mint_a, make_metadata(mint_a, "AAA")),
            (mint_b, make_metadata(mint_b, "BBB")),
        ])),
        Arc::new(MockPriceRepo::empty()),
    );

    let agg = svc.get_token(&mint_b).await.unwrap().unwrap();
    assert_eq!(agg.metadata.mint, mint_b);
    assert_eq!(agg.metadata.symbol, Some("BBB".to_string()));
}

#[tokio::test]
async fn metadata_repo_error_propagates() {
    let mint = pk(20);

    let svc = TokenService::new(
        Arc::new(MockMetadataRepo::failing()),
        Arc::new(MockPriceRepo::empty()),
    );

    assert!(matches!(
        svc.get_token(&mint).await.unwrap_err(),
        RepositoryError::Integrity(_)
    ));
}

#[tokio::test]
async fn price_repo_error_propagates() {
    let mint = pk(20);

    let svc = TokenService::new(
        Arc::new(MockMetadataRepo::with(vec![(
            mint,
            make_metadata(mint, "SOL"),
        )])),
        Arc::new(MockPriceRepo::failing()),
    );

    assert!(matches!(
        svc.get_token(&mint).await.unwrap_err(),
        RepositoryError::Integrity(_)
    ));
}

/// Mutation: the window's start recomputed on every page — a later page's start slides with
/// the clock and drops the prices between.
#[tokio::test]
async fn a_later_page_keeps_the_start_of_its_cursor() {
    let mint = pk(20);
    let prices = Arc::new(MockPriceRepo::empty());
    let svc = TokenService::new(Arc::new(MockMetadataRepo::empty()), prices.clone());
    let cursor = yog_core::domain::TokenPriceCursor {
        mint,
        from: crate::testing::ts(1_000),
        fetched_at: crate::testing::ts(2_000),
    };

    let before = chrono::Utc::now();
    svc.list_token_prices(&mint, yog_core::domain::Window::Day, None, 50)
        .await
        .unwrap();
    svc.list_token_prices(&mint, yog_core::domain::Window::Day, Some(cursor), 50)
        .await
        .unwrap();

    let starts = prices.windows_from.lock().unwrap().clone();
    assert!(
        starts[0] >= before - chrono::Duration::hours(24),
        "{starts:?}"
    );
    assert!(starts[0] <= chrono::Utc::now() - chrono::Duration::hours(24));
    assert_eq!(starts[1], crate::testing::ts(1_000));
}
