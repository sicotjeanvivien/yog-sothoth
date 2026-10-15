use yog_core::domain;

use super::kind_response::LiquidityEventKindResponse;
use crate::testing::wire_tag;

#[test]
fn kind_matches_the_domain() {
    use domain::MeteoraDammV2LiquidityEventKind::*;
    for v in [Add, Remove] {
        assert_eq!(wire_tag(LiquidityEventKindResponse::from(v)), v.as_str());
    }
}
