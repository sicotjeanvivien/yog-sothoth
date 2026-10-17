use super::ProtocolResponse;
use crate::testing::wire_tag;

#[test]
fn protocol_matches_the_domain() {
    for p in yog_core::domain::Protocol::all() {
        assert_eq!(wire_tag(ProtocolResponse::from(*p)), p.as_str());
    }
}
