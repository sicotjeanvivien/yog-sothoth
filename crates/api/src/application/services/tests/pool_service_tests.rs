//! Unit tests for `PoolService::get_pool`: a pool reads its own protocol's properties, and no
//! other's.

use std::sync::Arc;

use yog_core::domain::{MeteoraDammV2PoolProperties, PoolEntry, PoolValue, Protocol};

use super::{PoolProperties, PoolService};
use crate::testing::{MockDammV2PropertiesRepo, MockPoolEntriesRepo, make_pool, pk};

fn entry(seed: u8, protocol: Protocol) -> PoolEntry {
    PoolEntry {
        pool: yog_core::domain::Pool {
            protocol,
            ..make_pool(pk(seed), pk(100), pk(101))
        },
        token_a: None,
        token_b: None,
        state: None,
        value: PoolValue::default(),
    }
}

fn properties(seed: u8) -> MeteoraDammV2PoolProperties {
    MeteoraDammV2PoolProperties {
        pool_address: pk(seed),
        protocol_fee_percent: Some(20),
        referral_fee_percent: Some(20),
        base_fee_kind: Some("constant".to_owned()),
        has_dynamic_fee: Some(false),
        fee_scheduler: None,
    }
}

/// Mutation: the satellite read whatever the protocol — a DLMM pool asks the DAMM v2 table; or
/// never read — a DAMM v2 pool without its block.
#[tokio::test]
async fn a_pool_reads_its_own_protocols_properties_only() {
    let satellite = Arc::new(MockDammV2PropertiesRepo {
        properties: vec![properties(1)],
        ..MockDammV2PropertiesRepo::default()
    });
    let svc = PoolService::new(
        Arc::new(MockPoolEntriesRepo {
            entries: vec![
                entry(1, Protocol::MeteoraDammV2),
                entry(2, Protocol::MeteoraDlmm),
            ],
        }),
        satellite.clone(),
    );

    let damm = svc.get_pool(pk(1)).await.unwrap().unwrap();
    let dlmm = svc.get_pool(pk(2)).await.unwrap().unwrap();
    let unknown = svc.get_pool(pk(3)).await.unwrap();

    assert!(matches!(
        damm.properties,
        Some(PoolProperties::MeteoraDammV2(ref p)) if p.pool_address == pk(1)
    ));
    assert!(dlmm.properties.is_none());
    assert!(unknown.is_none());
    assert_eq!(*satellite.reads.lock().unwrap(), vec![pk(1)]);
}
