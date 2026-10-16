use rust_decimal::Decimal;
use yog_core::domain::{self, PoolSideValues};

use super::PoolCurrentStateResponse;
use super::last_event_kind_response::LastEventKindResponse;
use crate::application::PoolCurrentStateView;
use crate::testing::{make_pool_current_state, pk, wire_tag};

#[test]
fn last_event_kind_matches_the_domain() {
    use domain::LastEventKind::*;
    for v in [Swap, LiquidityAdd, LiquidityRemove] {
        assert_eq!(wire_tag(LastEventKindResponse::from(v)), v.as_str());
    }
}

#[test]
fn each_side_value_reaches_the_wire_under_its_own_name() {
    // Mutation: the two sides swapped, or one dropped, in the `From` mapping.
    let view = PoolCurrentStateView {
        state: make_pool_current_state(pk(1)),
        spot_price_a_in_b: None,
        side_values: PoolSideValues {
            value_a_usd: Some(Decimal::new(2050, 2)),
            value_b_usd: None,
        },
    };

    let json = serde_json::to_value(PoolCurrentStateResponse::from(view)).unwrap();

    assert_eq!(json["valueAUsd"], "20.50");
    assert!(json["valueBUsd"].is_null());
}
