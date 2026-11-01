use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{LiquidityFlows, LiquidityTotals, Protocol};

use super::ProtocolLiquidityFlowsResponse;

fn flows() -> LiquidityFlows {
    LiquidityFlows {
        created_usd: Decimal::from(1),
        added_usd: Decimal::from(2),
        removed_usd: Decimal::from(3),
        moves: 4,
        moves_valued: 5,
    }
}

/// Mutation: the sums exchanged, or the owners dropped — a withdrawal read as a deposit.
#[test]
fn each_flow_of_the_totals_is_under_its_name() {
    let totals = LiquidityTotals {
        flows: flows(),
        owners: 6,
    };

    assert_eq!(
        serde_json::to_value(ProtocolLiquidityFlowsResponse::new(
            Protocol::MeteoraDammV2,
            totals
        ))
        .unwrap(),
        json!({
            "protocol": Protocol::MeteoraDammV2.as_str(),
            "createdUsd": "1",
            "addedUsd": "2",
            "removedUsd": "3",
            "moves": 4,
            "movesValued": 5,
            "owners": 6,
        })
    );
}
