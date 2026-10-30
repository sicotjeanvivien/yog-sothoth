//! Unit tests for `ProtocolCatalogService`.

use std::sync::Arc;

use yog_core::{
    Cursor,
    domain::{Protocol, ProtocolPools},
};

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

/// Mutations: the page after the cursor starting at it, or its last entry given no cursor — a
/// protocol served twice, or the next page never asked.
#[tokio::test]
async fn the_catalogue_pages_without_repeat_nor_gap() {
    let svc = ProtocolCatalogService::new(
        Arc::new(PoolCountsRepo::by_protocol(vec![])),
        vec![],
        work_slots(),
    );

    let mut seen = Vec::new();
    let mut after = None;
    loop {
        let (page, _) = svc.list_page(after, 1).await.unwrap();
        assert_eq!(page.is_first, after.is_none());
        seen.extend(page.items.iter().map(|entry| entry.protocol));
        match page.next_cursor {
            Some(Cursor::Protocol(last)) => after = Some(last),
            Some(other) => panic!("{other:?}"),
            None => {
                assert!(page.is_last);
                break;
            }
        }
        assert!(seen.len() <= Protocol::all().len(), "{seen:?}");
    }

    assert_eq!(seen, Protocol::all());
}

/// Mutation: the pools seen summed over the page — a total that shrinks with `limit`.
#[tokio::test]
async fn the_pools_seen_count_every_protocol_whatever_the_page() {
    let svc = ProtocolCatalogService::new(
        Arc::new(PoolCountsRepo::by_protocol(vec![
            ProtocolPools {
                protocol: Protocol::MeteoraDammV2,
                pools_seen: 3,
            },
            ProtocolPools {
                protocol: Protocol::MeteoraDlmm,
                pools_seen: 4,
            },
        ])),
        vec![],
        work_slots(),
    );

    let (page, pools_seen) = svc.list_page(None, 1).await.unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(pools_seen, 7);
}
