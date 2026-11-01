use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{LastEventKind, PoolCurrentState, PoolValue};

use super::PoolStateResponse;

fn wire(kind: LastEventKind, value: PoolValue) -> serde_json::Value {
    serde_json::to_value(PoolStateResponse::new(
        PoolCurrentState {
            last_event_at: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
            last_event_kind: kind,
            reserve_a: 1,
            reserve_b: 2,
            last_swap_at: Some(Utc.with_ymd_and_hms(2026, 10, 25, 11, 0, 0).unwrap()),
        },
        value,
    ))
    .unwrap()
}

/// Mutation: the reserves or the two sides' values exchanged — a side valued as the other.
#[test]
fn each_value_of_the_state_is_under_its_name() {
    let value = PoolValue {
        value_a_usd: Some(Decimal::from(3)),
        value_b_usd: Some(Decimal::from(4)),
        tvl_usd: Some(Decimal::from(5)),
    };

    assert_eq!(
        wire(LastEventKind::Swap, value),
        json!({
            "lastEventAt": "2026-10-25T12:00:00Z",
            "lastEventKind": "swap",
            "reserveA": "1",
            "reserveB": "2",
            "lastSwapAt": "2026-10-25T11:00:00Z",
            "valueAUsd": "3",
            "valueBUsd": "4",
            "tvlUsd": "5",
        })
    );
}

/// Mutation: a side without a price answered as 0 — a pool not valued said to be worth nothing.
#[test]
fn an_unvalued_pool_stays_null() {
    let state = wire(LastEventKind::Swap, PoolValue::default());

    assert_eq!(state["valueAUsd"], serde_json::Value::Null);
    assert_eq!(state["valueBUsd"], serde_json::Value::Null);
    assert_eq!(state["tvlUsd"], serde_json::Value::Null);
}

/// Mutation: a kind mapped to another — a withdrawal published as a deposit.
#[test]
fn each_last_event_is_said() {
    for (kind, tag) in [
        (LastEventKind::LiquidityAdd, "liquidity_add"),
        (LastEventKind::LiquidityRemove, "liquidity_remove"),
    ] {
        assert_eq!(wire(kind, PoolValue::default())["lastEventKind"], tag);
    }
}
