//! Unit tests for the envelopes that add a `total`: two protocols, which the prod does not have.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::{Value, json, to_value};
use yog_core::domain::{
    FrozenHours, FrozenPeriod, LiquidityTurnover, Protocol, ProtocolBucket,
    ProtocolLiquidityTurnover, ProtocolPoolCreations, ProtocolSwapActivity, ProtocolTvlChange,
    SwapActivity, TvlReading, Window,
};

use super::{
    LiquidityTurnoverTotalsResponse, PoolCreationTotalsResponse, ProtocolSwapSeriesResponse,
    ProtocolSwapTotalsResponse, TvlTotalsResponse,
};
use crate::application::{Measured, SeriesInterval, SwapTotals};
use crate::http::dto::response::{
    ProtocolPoolCreationsResponse, ProtocolSwapActivityBucketResponse,
    ProtocolSwapActivityResponse, ProtocolTvlChangeResponse,
};
use crate::testing::at_october;

const BOTH: [Protocol; 2] = [Protocol::MeteoraDammV2, Protocol::MeteoraDlmm];

fn period() -> FrozenPeriod {
    FrozenPeriod::of(
        FrozenHours {
            first: at_october(1, 0),
            last_frozen: at_october(20, 12),
        },
        Window::Day,
        None,
    )
}

fn activity() -> SwapActivity {
    SwapActivity {
        swaps: 2,
        volume_usd: Some(Decimal::TEN),
        fees_usd: None,
        lp_fees_usd: None,
        protocol_fees_usd: None,
        referral_fees_usd: None,
        pool_hours: 3,
        pool_hours_valued: 1,
    }
}

fn measured<T>(items: Vec<T>) -> Measured<Vec<T>> {
    Measured {
        period: Some(period()),
        items,
    }
}

fn reading(at: DateTime<Utc>, tvl: i64) -> TvlReading {
    TvlReading {
        at,
        tvl_usd: Some(Decimal::from(tvl)),
    }
}

/// Mutation: a `total` dropped from its envelope, or read off one row: under one protocol the two
/// would look alike.
#[test]
fn each_envelope_sums_its_protocols_in_total() {
    let swaps = BOTH.map(|protocol| ProtocolSwapActivity {
        protocol,
        activity: activity(),
        pools: Some(4),
    });
    let totals = SwapTotals {
        measured: Measured {
            period: None,
            items: swaps.to_vec(),
        },
        previous: None,
    };
    let body = to_value(
        ProtocolSwapTotalsResponse::<ProtocolSwapActivityResponse>::new(Window::Day, None, totals),
    )
    .unwrap();
    assert_eq!(body["total"]["volumeUsd"], json!("20"));
    assert_eq!(body["total"]["pools"], json!(8));
    assert_eq!(body["items"].as_array().map(Vec::len), Some(2));
    assert_eq!(body["previous"], Value::Null);

    let at = at_october(20, 13);
    let series = BOTH.map(|protocol| ProtocolBucket {
        protocol,
        from: at,
        value: activity(),
    });
    let body = to_value(ProtocolSwapSeriesResponse::<
        ProtocolSwapActivityBucketResponse,
    >::new(
        Window::Day,
        SeriesInterval::Hour,
        measured(series.to_vec()),
    ))
    .unwrap();
    assert_eq!(body["total"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["total"][0]["volumeUsd"], json!("20"));

    let changes = BOTH.map(|protocol| ProtocolTvlChange {
        protocol,
        end: reading(at, 100),
        start: Some(reading(at, 90)),
        discovered_tvl_usd: Some(Decimal::ONE),
    });
    let body = to_value(TvlTotalsResponse::<ProtocolTvlChangeResponse>::new(
        Window::Day,
        measured(changes.to_vec()),
    ))
    .unwrap();
    assert_eq!(body["total"]["endTvlUsd"], json!("200"));
    assert_eq!(body["total"]["changeUsd"], json!("18"));

    let creations = BOTH.map(|protocol| ProtocolPoolCreations { protocol, pools: 5 });
    let body = to_value(
        PoolCreationTotalsResponse::<ProtocolPoolCreationsResponse>::new(
            Window::Day,
            measured(creations.to_vec()),
        ),
    )
    .unwrap();
    assert_eq!(body["total"]["pools"], json!(10));

    let turnover = LiquidityTurnover {
        turnover: Some(Decimal::TWO),
        pool_hours: 6,
        pool_hours_valued: 2,
    };
    let rows = BOTH.map(|protocol| ProtocolLiquidityTurnover { protocol, turnover });
    let body = to_value(LiquidityTurnoverTotalsResponse::new(
        Window::Day,
        measured(rows.to_vec()),
        Some(turnover),
    ))
    .unwrap();
    assert_eq!(body["total"]["turnover"], json!("2"));
    assert_eq!(body["coverageUnit"], json!("poolHours"));
}
