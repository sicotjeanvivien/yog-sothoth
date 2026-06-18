//! Unit tests for `StatsService`.

use std::sync::Arc;

use rust_decimal::Decimal;
use yog_core::{
    RepositoryError,
    domain::{GlobalAnalytics, PoolCounts},
};

use super::super::StatsService;
use crate::testing::{MockGlobalAnalyticsRepo, PoolCountsRepo};

fn analytics() -> GlobalAnalytics {
    GlobalAnalytics {
        total_tvl_usd: Some(Decimal::new(1_000, 0)),
        pools_priced: 42,
        volume_24h_usd: Some(Decimal::new(500, 0)),
        fees_24h_usd: Some(Decimal::new(7, 0)),
    }
}

#[tokio::test]
async fn composes_analytics_and_counts() {
    let svc = StatsService::new(
        Arc::new(MockGlobalAnalyticsRepo::with(analytics())),
        Arc::new(PoolCountsRepo::with(PoolCounts {
            observed: 359,
            discovered_24h: 55,
        })),
    );

    let agg = svc.get_stats().await.unwrap();

    assert_eq!(agg.analytics.pools_priced, 42);
    assert_eq!(agg.analytics.total_tvl_usd, Some(Decimal::new(1_000, 0)));
    assert_eq!(agg.counts.observed, 359);
    assert_eq!(agg.counts.discovered_24h, 55);
}

#[tokio::test]
async fn analytics_repo_error_propagates() {
    let svc = StatsService::new(
        Arc::new(MockGlobalAnalyticsRepo::failing()),
        // Counts repo must not be reached if analytics fails first.
        Arc::new(PoolCountsRepo::with(PoolCounts {
            observed: 1,
            discovered_24h: 0,
        })),
    );

    assert!(matches!(
        svc.get_stats().await.unwrap_err(),
        RepositoryError::Integrity(_)
    ));
}

#[tokio::test]
async fn counts_repo_error_propagates() {
    let svc = StatsService::new(
        Arc::new(MockGlobalAnalyticsRepo::with(analytics())),
        Arc::new(PoolCountsRepo::failing()),
    );

    assert!(matches!(
        svc.get_stats().await.unwrap_err(),
        RepositoryError::Integrity(_)
    ));
}
