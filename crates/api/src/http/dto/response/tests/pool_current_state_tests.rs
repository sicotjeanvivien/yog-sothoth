use yog_core::domain;

use super::last_event_kind_response::LastEventKindResponse;
use crate::testing::wire_tag;

#[test]
fn last_event_kind_matches_the_domain() {
    use domain::LastEventKind::*;
    for v in [Swap, LiquidityAdd, LiquidityRemove] {
        assert_eq!(wire_tag(LastEventKindResponse::from(v)), v.as_str());
    }
}
