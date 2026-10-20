//! Unit tests for `ProtocolCatalogService`.

use std::sync::Arc;

use yog_core::domain::{Protocol, ProtocolPools};

use super::super::ProtocolCatalogService;
use crate::testing::{PoolCountsRepo, work_slots};

/// Mutations: a protocol without a pool left out, or every protocol said observed.
#[tokio::test]
async fn every_protocol_is_listed_observed_or_not() {
    let svc = ProtocolCatalogService::new(
        Arc::new(PoolCountsRepo::by_protocol(vec![ProtocolPools {
            protocol: Protocol::MeteoraDammV2,
            pools_seen: 112_581,
        }])),
        vec![Protocol::MeteoraDammV2],
        work_slots(),
    );

    let entries = svc.list().await.unwrap();

    assert_eq!(entries.len(), Protocol::all().len());
    let dlmm = entries
        .iter()
        .find(|e| e.protocol == Protocol::MeteoraDlmm)
        .unwrap();
    assert!(!dlmm.observed);
    assert_eq!(dlmm.pools_seen, 0);
    let damm = entries
        .iter()
        .find(|e| e.protocol == Protocol::MeteoraDammV2)
        .unwrap();
    assert!(damm.observed);
    assert_eq!(damm.pools_seen, 112_581);
}
