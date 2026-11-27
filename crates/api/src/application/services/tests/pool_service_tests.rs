//! Unit tests for `PoolService::get_pool`: a pool reads its own protocol's properties, and no
//! other's.

use std::sync::Arc;

use yog_core::domain::{
    FeeSchedulerDrop, FieldOutOfRange, MeteoraDammV2PoolProperties, PoolEntry, PoolValue, Protocol,
};

use super::{PoolProperties, PoolService};
use crate::testing::{
    MockDammV2PropertiesRepo, MockPoolEntriesRepo, counter, make_pool, pk, recorded,
};

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
        fee_scheduler_dropped: None,
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

const DEGRADED: &str = "yog_api_pool_properties_degraded_total";

/// Mutation: the drop left uncounted, counted under the other reason, or a whole pool counted.
#[test]
fn a_fee_curve_served_without_its_stored_value_is_counted_under_its_reason() {
    let out_of_range = MeteoraDammV2PoolProperties {
        fee_scheduler_dropped: Some(FeeSchedulerDrop::OutOfRange(FieldOutOfRange {
            field: "period_frequency",
        })),
        ..properties(1)
    };
    let partly_null = MeteoraDammV2PoolProperties {
        fee_scheduler_dropped: Some(FeeSchedulerDrop::PartlyNull),
        ..properties(2)
    };
    let svc = PoolService::new(
        Arc::new(MockPoolEntriesRepo {
            entries: vec![
                entry(1, Protocol::MeteoraDammV2),
                entry(2, Protocol::MeteoraDammV2),
                entry(3, Protocol::MeteoraDammV2),
            ],
        }),
        Arc::new(MockDammV2PropertiesRepo {
            properties: vec![out_of_range, partly_null, properties(3)],
            ..MockDammV2PropertiesRepo::default()
        }),
    );

    let (_, counters) = recorded(async {
        for seed in [1, 1, 2, 3] {
            svc.get_pool(pk(seed)).await.unwrap().unwrap();
        }
    });

    let reason = |r| {
        counter(
            &counters,
            DEGRADED,
            &[("protocol", "meteora_damm_v2"), ("reason", r)],
        )
    };
    assert_eq!(
        reason("fee_scheduler_out_of_range"),
        Some(2),
        "two reads, one row"
    );
    assert_eq!(reason("fee_scheduler_partly_null"), Some(1));
}
