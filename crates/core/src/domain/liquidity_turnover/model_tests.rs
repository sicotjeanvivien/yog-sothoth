use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;

use crate::domain::Protocol;

fn tvl_reading(protocol: Protocol, tvl: Option<i64>) -> crate::domain::ProtocolTvl {
    crate::domain::ProtocolTvl {
        protocol,
        at: Utc.with_ymd_and_hms(2026, 10, 25, 3, 0, 0).unwrap(),
        tvl_usd: tvl.map(Decimal::from),
        pools_priced: 1,
        pools_unvalued: 0,
        protocol_part: None,
    }
}

fn swaps(protocol: Protocol, volume: Option<i64>) -> crate::domain::ProtocolSwapActivity {
    crate::domain::ProtocolSwapActivity {
        protocol,
        activity: crate::domain::SwapActivity {
            swaps: 1,
            volume_usd: volume.map(Decimal::from),
            fees_usd: None,
            lp_fees_usd: None,
            protocol_fees_usd: None,
            referral_fees_usd: None,
            pool_hours: 10,
            pool_hours_valued: 9,
            protocol_part: None,
        },
        pools: None,
    }
}

/// Mutation: the total the sum of the ratios, 5 here — a turnover no protocol has.
#[test]
fn the_total_turnover_is_every_volume_over_every_tvl() {
    let total = super::LiquidityTurnover::total(
        &[
            swaps(Protocol::MeteoraDammV2, Some(100)),
            swaps(Protocol::MeteoraDlmm, Some(50)),
        ],
        &[
            tvl_reading(Protocol::MeteoraDammV2, Some(50)),
            tvl_reading(Protocol::MeteoraDlmm, Some(25)),
        ],
    )
    .unwrap();

    assert_eq!(total.turnover, Some(Decimal::TWO));
    assert_eq!(total.pool_hours, 20);
    assert_eq!(total.pool_hours_valued, 18);
}

/// Mutation: a TVL of 0 divided, a TVL unknown read as 0, or a row given another protocol's TVL.
#[test]
fn a_turnover_is_unknown_without_a_tvl_above_zero() {
    let rows = super::ProtocolLiquidityTurnover::rows(
        &[
            swaps(Protocol::MeteoraDammV2, Some(100)),
            swaps(Protocol::MeteoraDlmm, Some(50)),
        ],
        &[
            tvl_reading(Protocol::MeteoraDlmm, Some(0)),
            tvl_reading(Protocol::MeteoraDammV2, Some(25)),
        ],
    );

    let turnovers: Vec<_> = rows.iter().map(|row| row.turnover.turnover).collect();
    assert_eq!(turnovers, [Some(Decimal::from(4)), None]);
    assert_eq!(
        rows[0].tvl_at,
        Some(tvl_reading(Protocol::MeteoraDammV2, None).at)
    );
    let below = super::ProtocolLiquidityTurnover::rows(
        &[swaps(Protocol::MeteoraDlmm, Some(50))],
        &[tvl_reading(Protocol::MeteoraDlmm, Some(-1))],
    );
    assert_eq!(below[0].turnover.turnover, None);
    let unread =
        super::ProtocolLiquidityTurnover::rows(&[swaps(Protocol::MeteoraDlmm, Some(50))], &[]);
    assert_eq!(unread[0].turnover.turnover, None);
    assert_eq!(unread[0].tvl_at, None);
    assert_eq!(
        super::LiquidityTurnover::total(&[swaps(Protocol::MeteoraDammV2, Some(1))], &[])
            .unwrap()
            .turnover,
        None
    );
}
