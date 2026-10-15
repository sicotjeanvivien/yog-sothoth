use yog_core::domain;

use super::PriceProviderResponse;
use crate::testing::wire_tag;

#[test]
fn price_provider_matches_the_domain() {
    use domain::PriceProvider::*;
    for v in [Jupiter, Helius, Fallback] {
        assert_eq!(wire_tag(PriceProviderResponse::from(v)), v.as_str());
    }
}
