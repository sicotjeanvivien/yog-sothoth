use yog_core::domain;

use super::trade_direction_response::TradeDirectionResponse;
use crate::testing::wire_tag;

#[test]
fn trade_direction_matches_the_domain() {
    use domain::TradeDirection::*;
    for v in [AtoB, BtoA] {
        assert_eq!(wire_tag(TradeDirectionResponse::from(v)), v.as_str());
    }
}
