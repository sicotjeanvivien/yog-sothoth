use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use yog_core::domain::{ClaimTotals, Protocol, ProtocolBucket};

use super::{ProtocolClaimBucketResponse, ProtocolClaimTotalsResponse};

fn totals() -> ClaimTotals {
    ClaimTotals {
        claims: 1,
        claims_valued: 2,
        amount_usd: Decimal::from(3),
        owners: 4,
    }
}

/// Mutation: two counts exchanged, or the bucket's start dropped — the claims read as the owners.
#[test]
fn each_value_of_the_claims_is_under_its_name() {
    let expected = json!({
        "protocol": Protocol::MeteoraDammV2.as_str(),
        "claims": 1,
        "claimsValued": 2,
        "amountUsd": "3",
        "owners": 4,
    });

    let total = ProtocolClaimTotalsResponse::new(Protocol::MeteoraDammV2, totals());
    assert_eq!(serde_json::to_value(total).unwrap(), expected);

    let bucket = ProtocolClaimBucketResponse::from(ProtocolBucket {
        protocol: Protocol::MeteoraDammV2,
        from: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        value: totals(),
    });
    let mut with_from = expected;
    with_from["from"] = json!("2026-10-25T12:00:00Z");
    assert_eq!(serde_json::to_value(bucket).unwrap(), with_from);
}
